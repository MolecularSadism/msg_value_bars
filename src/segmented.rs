//! Segmented (discrete-slot) value bars.
//!
//! A [`SegmentedBar`] shows a value as N slots filling in order, driven by
//! the same [`CircularBarValue`] binding the circular kinds use. The plugin
//! spawns one bare child entity per slot carrying a [`Segment`] and keeps its
//! [`SegmentState`] current:
//!
//! * slots below the value are [`Fill`](SegmentState::Fill),
//! * slots above it are [`Empty`](SegmentState::Empty),
//! * a partially-covered slot blinks between
//!   [`Empty`](SegmentState::Empty) and [`Follow`](SegmentState::Follow) at
//!   the bar's configured timing (the classic "this one is in play" pulse).
//!
//! The crate owns the state machine, not the pixels: segments carry no
//! visuals. A host themes them per slot by attaching its own render
//! components to the children and restyling on `Changed<Segment>` — atlas
//! sprites, UI nodes, whatever it draws with — using
//! [`SegmentedBar::display_index`] to place a slot under either fill
//! direction. Order restyle systems `.after(ValueBarSystems::Sync)` in
//! `Update` (the crate exports [`ValueBarSystems`](crate::ValueBarSystems)
//! for this) so they see the frame's final states rather than picking them
//! up a frame late.

use std::time::Duration;

use bevy::prelude::*;

use crate::CircularBarValue;

/// Fill direction of a segmented bar: which end slot `0` sits at when the
/// host lays the slots out.
///
/// Purely a layout hint carried for the theming layer —
/// [`SegmentedBar::display_index`] applies it. Slot *filling* order is always
/// by [`Segment::index`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum SegmentFillDirection {
    /// Slot 0 first: left-to-right (or top-to-bottom) layouts.
    #[default]
    Normal,
    /// Slot 0 last: right-to-left (or bottom-to-top) layouts.
    Inverse,
}

/// State of an individual segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum SegmentState {
    /// Slot is empty.
    #[default]
    Empty,
    /// Slot is fully filled.
    Fill,
    /// Slot is in follow state (the blink phase of a partial slot; hosts may
    /// also use it for damage/recovery styling).
    Follow,
    /// Slot is in an indeterminate/loading state. Never set by the plugin,
    /// but not preserved either: the state update stomps host-set states
    /// (this one included) back to `Fill`/`Empty` the next time the bar's
    /// value, configuration, or children change, so a host-set state persists
    /// only until then.
    Indeterminate,
}

/// A segmented value bar: `slot_count` discrete slots driven by the entity's
/// [`CircularBarValue`] (0..1, so each slot spans `1 / slot_count` of it).
///
/// [`CircularBarValue`] is a required component: spawning a bar without one
/// inserts the default (zero) value. The plugin spawns the segment children
/// and keeps their states current.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
#[require(CircularBarValue)]
pub struct SegmentedBar {
    /// Number of slots. May be changed on a live bar; the plugin spawns or
    /// despawns segment children to match.
    pub slot_count: usize,
    /// Layout hint for the theming layer; see [`SegmentFillDirection`].
    pub fill_direction: SegmentFillDirection,
    /// Duration of the empty phase of a partial slot's blink.
    pub blink_empty: Duration,
    /// Duration of the follow phase of a partial slot's blink.
    pub blink_follow: Duration,
}

impl Default for SegmentedBar {
    fn default() -> Self {
        Self {
            slot_count: 5,
            fill_direction: SegmentFillDirection::default(),
            blink_empty: Duration::from_millis(50),
            blink_follow: Duration::from_millis(50),
        }
    }
}

impl SegmentedBar {
    /// Tolerance, as a fraction of one slot, within which a value counts as
    /// landing exactly on a slot boundary in [`Self::slot_split`].
    pub const BOUNDARY_EPSILON: f32 = 1e-4;

    /// A bar with `slot_count` slots.
    #[must_use]
    pub fn new(slot_count: usize) -> Self {
        Self {
            slot_count,
            ..Default::default()
        }
    }

    /// The number of slots needed to represent `max_value` at
    /// `value_per_slot` units each — `ceil(max / per_slot)`, or 0 when
    /// `value_per_slot` is not positive.
    #[must_use]
    pub fn from_values(max_value: f32, value_per_slot: f32) -> Self {
        let slot_count = if value_per_slot <= 0.0 {
            0
        } else {
            (max_value / value_per_slot).ceil() as usize
        };
        Self::new(slot_count)
    }

    /// Sets the fill direction.
    #[must_use]
    pub fn with_fill_direction(mut self, fill_direction: SegmentFillDirection) -> Self {
        self.fill_direction = fill_direction;
        self
    }

    /// Sets the blink timing for partially-covered slots.
    #[must_use]
    pub fn with_blink_timing(mut self, empty: Duration, follow: Duration) -> Self {
        self.blink_empty = empty;
        self.blink_follow = follow;
        self
    }

    /// Where slot `index` sits in the host's layout under the bar's fill
    /// direction: `index` itself for [`Normal`](SegmentFillDirection::Normal),
    /// mirrored for [`Inverse`](SegmentFillDirection::Inverse).
    #[must_use]
    pub fn display_index(&self, index: usize) -> usize {
        match self.fill_direction {
            SegmentFillDirection::Normal => index,
            SegmentFillDirection::Inverse => self.slot_count.saturating_sub(1 + index),
        }
    }

    /// Threshold split for a normalized value: `(full_slots, has_partial)`.
    ///
    /// `full_slots` slots are completely covered; when `has_partial` the next
    /// slot is partially covered (and should blink). A value landing exactly
    /// on a slot boundary has no partial slot; the boundary test uses a small
    /// epsilon ([`Self::BOUNDARY_EPSILON`], in slot units) on either side, so
    /// f32 rounding in host math (e.g. `hp / max_hp`) landing a hair off a
    /// boundary cannot make a conceptually-full slot blink.
    #[must_use]
    pub fn slot_split(&self, value: f32) -> (usize, bool) {
        if self.slot_count == 0 {
            return (0, false);
        }
        let units = value.clamp(0.0, 1.0) * self.slot_count as f32;
        let mut full = units.floor() as usize;
        let mut partial = units - full as f32;
        if partial > 1.0 - Self::BOUNDARY_EPSILON {
            full += 1;
            partial = 0.0;
        }
        (
            full,
            partial > Self::BOUNDARY_EPSILON && full < self.slot_count,
        )
    }
}

/// One slot of a [`SegmentedBar`], spawned as a bare child entity.
///
/// Hosts attach their own render components to these children and restyle on
/// `Changed<Segment>`.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct Segment {
    /// Index of this slot within the bar (0-based, filling order).
    pub index: usize,
    /// Current state of the slot.
    pub state: SegmentState,
}

/// Blink bookkeeping for the partially-covered slot.
///
/// Inserted and removed by the plugin; while present, the segment's state
/// alternates between [`SegmentState::Empty`] and [`SegmentState::Follow`].
/// The phase durations are refreshed from the bar whenever it changes, taking
/// effect at the next phase flip.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct SegmentBlink {
    /// Time left in the current phase.
    pub timer: Timer,
    /// Whether currently showing the empty phase (vs the follow phase).
    pub showing_empty: bool,
    /// Duration of the empty phase.
    pub empty_duration: Duration,
    /// Duration of the follow phase.
    pub follow_duration: Duration,
}

impl SegmentBlink {
    /// A blink starting in its empty phase.
    #[must_use]
    pub fn new(empty: Duration, follow: Duration) -> Self {
        Self {
            timer: Timer::new(empty, TimerMode::Once),
            showing_empty: true,
            empty_duration: empty,
            follow_duration: follow,
        }
    }

    /// The visual state of the current blink phase.
    #[must_use]
    pub fn current_state(&self) -> SegmentState {
        if self.showing_empty {
            SegmentState::Empty
        } else {
            SegmentState::Follow
        }
    }
}

/// Spawns and reconciles the bare segment children of bars: a newly-added
/// bar gets one child per slot, and a later `slot_count` change spawns the
/// missing slots and despawns the extras. Non-[`Segment`] children are left
/// alone.
pub(crate) fn spawn_segments(
    mut commands: Commands,
    bars: Query<(Entity, &SegmentedBar, Option<&Children>), Changed<SegmentedBar>>,
    segments: Query<&Segment>,
) {
    for (entity, bar, children) in &bars {
        let mut present = vec![false; bar.slot_count];
        for child in children.into_iter().flat_map(|children| children.iter()) {
            let Ok(segment) = segments.get(child) else {
                continue;
            };
            if let Some(slot) = present.get_mut(segment.index) {
                *slot = true;
            } else {
                commands.entity(child).despawn();
            }
        }
        for index in (0..bar.slot_count).filter(|&index| !present[index]) {
            commands.entity(entity).with_child(Segment {
                index,
                state: SegmentState::Empty,
            });
        }
    }
}

/// Updates segment states from the bar's [`CircularBarValue`] and manages the
/// partial slot's [`SegmentBlink`].
///
/// Writes every non-blinking slot's state as [`Fill`](SegmentState::Fill) or
/// [`Empty`](SegmentState::Empty), stomping any host-set state (see
/// [`SegmentState::Indeterminate`]).
pub(crate) fn update_segment_states(
    mut commands: Commands,
    bars: Query<
        (&SegmentedBar, &CircularBarValue, &Children),
        Or<(
            Changed<CircularBarValue>,
            Changed<SegmentedBar>,
            Changed<Children>,
        )>,
    >,
    mut segments: Query<(&mut Segment, Option<&mut SegmentBlink>)>,
) {
    for (bar, value, children) in &bars {
        let (full, has_partial) = bar.slot_split(value.value);

        for child in children.iter() {
            let Ok((mut segment, blink)) = segments.get_mut(child) else {
                continue;
            };

            let should_blink = has_partial && segment.index == full;
            match blink {
                Some(mut blink) if should_blink => {
                    // Keep an already-blinking slot on the bar's current
                    // timing; the tick system applies it at the next flip.
                    if blink.empty_duration != bar.blink_empty {
                        blink.empty_duration = bar.blink_empty;
                    }
                    if blink.follow_duration != bar.blink_follow {
                        blink.follow_duration = bar.blink_follow;
                    }
                }
                None if should_blink => {
                    commands
                        .entity(child)
                        .insert(SegmentBlink::new(bar.blink_empty, bar.blink_follow));
                }
                Some(_) => {
                    commands.entity(child).remove::<SegmentBlink>();
                }
                None => {}
            }

            // The blink system owns a blinking slot's state.
            if should_blink {
                continue;
            }
            let new_state = if segment.index < full {
                SegmentState::Fill
            } else {
                SegmentState::Empty
            };
            if segment.state != new_state {
                segment.state = new_state;
            }
        }
    }
}

/// Ticks partial-slot blinks and writes the phase into the segment's state,
/// so hosts restyle blinking slots off the same `Changed<Segment>` signal.
///
/// Time left over when a phase ends is carried into the next phase (crossing
/// as many phases as the frame's delta covers), so the blink frequency is
/// independent of frame rate.
pub(crate) fn tick_segment_blink(
    time: Res<Time>,
    mut segments: Query<(&mut Segment, &mut SegmentBlink)>,
) {
    for (mut segment, mut blink) in &mut segments {
        let mut delta = time.delta();
        while !delta.is_zero() {
            let remaining = blink.timer.remaining();
            if delta < remaining {
                blink.timer.tick(delta);
                break;
            }
            delta -= remaining;
            blink.showing_empty = !blink.showing_empty;
            let next = if blink.showing_empty {
                blink.empty_duration
            } else {
                blink.follow_duration
            };
            blink.timer = Timer::new(next, TimerMode::Once);
            // An all-zero timing can never consume the delta; leave the
            // phase wherever the flip landed.
            if blink.empty_duration.is_zero() && blink.follow_duration.is_zero() {
                break;
            }
        }
        let state = blink.current_state();
        if segment.state != state {
            segment.state = state;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::time::TimeUpdateStrategy;

    use super::*;

    #[test]
    fn slot_count_calculation() {
        assert_eq!(SegmentedBar::from_values(100.0, 20.0).slot_count, 5);
        assert_eq!(SegmentedBar::from_values(100.0, 25.0).slot_count, 4);
        // 100/30 = 3.33, ceil = 4
        assert_eq!(SegmentedBar::from_values(100.0, 30.0).slot_count, 4);
        assert_eq!(SegmentedBar::from_values(100.0, 0.0).slot_count, 0);
    }

    #[test]
    fn builder_pattern() {
        let bar = SegmentedBar::from_values(50.0, 10.0)
            .with_fill_direction(SegmentFillDirection::Inverse)
            .with_blink_timing(Duration::from_millis(100), Duration::from_millis(200));

        assert_eq!(bar.slot_count, 5);
        assert_eq!(bar.fill_direction, SegmentFillDirection::Inverse);
        assert_eq!(bar.blink_empty, Duration::from_millis(100));
        assert_eq!(bar.blink_follow, Duration::from_millis(200));
    }

    #[test]
    fn display_index_follows_fill_direction() {
        let normal = SegmentedBar::new(4);
        assert_eq!(normal.display_index(0), 0);
        assert_eq!(normal.display_index(3), 3);

        let inverse = SegmentedBar::new(4).with_fill_direction(SegmentFillDirection::Inverse);
        assert_eq!(inverse.display_index(0), 3);
        assert_eq!(inverse.display_index(3), 0);
    }

    #[test]
    fn slot_split_threshold_semantics() {
        let bar = SegmentedBar::new(4);
        // Empty and full: no partial slot.
        assert_eq!(bar.slot_split(0.0), (0, false));
        assert_eq!(bar.slot_split(1.0), (4, false));
        // Exactly on a boundary: no partial slot.
        assert_eq!(bar.slot_split(0.5), (2, false));
        // Between boundaries: the next slot is partial.
        assert_eq!(bar.slot_split(0.625), (2, true));
        assert_eq!(bar.slot_split(0.1), (0, true));
        // Out-of-range values clamp.
        assert_eq!(bar.slot_split(2.0), (4, false));
        assert_eq!(bar.slot_split(-1.0), (0, false));
    }

    #[test]
    fn slot_split_snaps_near_boundary_values() {
        // A value one rounding error off a boundary must not report a
        // partial slot; boundary math is epsilon-tolerant on both sides.
        let bar = SegmentedBar::new(4);
        assert_eq!(bar.slot_split(0.500_001), (2, false));
        assert_eq!(bar.slot_split(0.499_999), (2, false));

        // A non-representable fraction from ordinary game math.
        let thirds = SegmentedBar::new(3);
        assert_eq!(thirds.slot_split(1.0 / 3.0), (1, false));
        assert_eq!(thirds.slot_split(2.0 / 3.0), (2, false));

        // Genuinely-partial values still blink.
        assert_eq!(bar.slot_split(0.51), (2, true));
    }

    #[test]
    fn blinking_state_toggle() {
        let mut blink = SegmentBlink::new(Duration::from_millis(50), Duration::from_millis(50));

        assert!(blink.showing_empty);
        assert_eq!(blink.current_state(), SegmentState::Empty);

        blink.showing_empty = false;
        assert_eq!(blink.current_state(), SegmentState::Follow);
    }

    fn segment_states(app: &mut App, bar_entity: Entity) -> Vec<(usize, SegmentState, bool)> {
        let mut states: Vec<(usize, SegmentState, bool)> = {
            let world = app.world_mut();
            let children: Vec<Entity> = world
                .get::<Children>(bar_entity)
                .map(|c| c.iter().collect())
                .unwrap_or_default();
            children
                .into_iter()
                .filter_map(|child| {
                    let blinking = world.get::<SegmentBlink>(child).is_some();
                    world
                        .get::<Segment>(child)
                        .map(|s| (s.index, s.state, blinking))
                })
                .collect()
        };
        states.sort_by_key(|(index, _, _)| *index);
        states
    }

    fn segment_entity(app: &App, bar: Entity, index: usize) -> Entity {
        let world = app.world();
        world
            .get::<Children>(bar)
            .unwrap()
            .iter()
            .find(|&child| world.get::<Segment>(child).map(|s| s.index) == Some(index))
            .unwrap()
    }

    fn test_app() -> App {
        // The full ValueBarPlugin needs asset infrastructure for the circular
        // kinds' material; segmented bars only need their own systems, added
        // here in the same order the plugin chains them.
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(
            Update,
            (spawn_segments, update_segment_states, tick_segment_blink).chain(),
        );
        app
    }

    #[test]
    fn segments_fill_up_to_the_value_and_partial_blinks() {
        let mut app = test_app();
        let bar = app
            .world_mut()
            .spawn((SegmentedBar::new(4), CircularBarValue::new(0.5)))
            .id();
        app.update(); // spawn children
        app.update(); // initial state pass sees the new Children

        // 0.5 of 4 slots = exactly 2 full, no partial.
        let states = segment_states(&mut app, bar);
        assert_eq!(
            states,
            vec![
                (0, SegmentState::Fill, false),
                (1, SegmentState::Fill, false),
                (2, SegmentState::Empty, false),
                (3, SegmentState::Empty, false),
            ]
        );

        // 0.625 of 4 slots = 2.5: slot 2 is partial and blinks.
        app.world_mut()
            .get_mut::<CircularBarValue>(bar)
            .unwrap()
            .set(0.625);
        app.update();
        app.update(); // blink component lands, blink system writes its phase

        let states = segment_states(&mut app, bar);
        assert_eq!(states[0], (0, SegmentState::Fill, false));
        assert_eq!(states[1], (1, SegmentState::Fill, false));
        assert!(states[2].2, "the partial slot carries a blink");
        assert!(
            matches!(states[2].1, SegmentState::Empty | SegmentState::Follow),
            "a blinking slot shows one of its two phases, got {:?}",
            states[2].1
        );
        assert_eq!(states[3], (3, SegmentState::Empty, false));
    }

    #[test]
    fn dropping_the_value_retracts_fill_and_stops_blinking() {
        let mut app = test_app();
        let bar = app
            .world_mut()
            .spawn((SegmentedBar::new(4), CircularBarValue::new(0.625)))
            .id();
        app.update();
        app.update();
        assert!(
            segment_states(&mut app, bar)[2].2,
            "slot 2 blinks at value 0.625"
        );

        // Down to exactly one slot: blink removed, states settle.
        app.world_mut()
            .get_mut::<CircularBarValue>(bar)
            .unwrap()
            .set(0.25);
        app.update();
        app.update();

        let states = segment_states(&mut app, bar);
        assert_eq!(
            states,
            vec![
                (0, SegmentState::Fill, false),
                (1, SegmentState::Empty, false),
                (2, SegmentState::Empty, false),
                (3, SegmentState::Empty, false),
            ]
        );
    }

    #[test]
    fn blink_carries_overshoot_across_phases() {
        let mut app = test_app();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        let bar = app
            .world_mut()
            .spawn((SegmentedBar::new(4), CircularBarValue::new(0.625)))
            .id();
        app.update();
        app.update();

        let slot = segment_entity(&app, bar, 2);
        assert_eq!(
            app.world().get::<Segment>(slot).unwrap().state,
            SegmentState::Empty,
            "a fresh blink starts in its empty phase"
        );

        // One 120 ms frame crosses both 50 ms phases and lands 20 ms into
        // the next empty phase: two flips, with the overshoot carried.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            120,
        )));
        app.update();
        assert_eq!(
            app.world().get::<Segment>(slot).unwrap().state,
            SegmentState::Empty
        );
        let blink = app.world().get::<SegmentBlink>(slot).unwrap();
        assert!(blink.showing_empty);
        assert_eq!(blink.timer.elapsed(), Duration::from_millis(20));

        // A 60 ms frame finishes that phase (30 ms left) and carries the
        // remaining 30 ms into the follow phase.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            60,
        )));
        app.update();
        assert_eq!(
            app.world().get::<Segment>(slot).unwrap().state,
            SegmentState::Follow
        );
        let blink = app.world().get::<SegmentBlink>(slot).unwrap();
        assert_eq!(blink.timer.elapsed(), Duration::from_millis(30));
    }

    #[test]
    fn changing_blink_timing_applies_to_a_live_blink() {
        let mut app = test_app();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        let bar = app
            .world_mut()
            .spawn((SegmentedBar::new(4), CircularBarValue::new(0.625)))
            .id();
        app.update();
        app.update();
        let slot = segment_entity(&app, bar, 2);

        app.world_mut()
            .get_mut::<SegmentedBar>(bar)
            .unwrap()
            .blink_follow = Duration::from_millis(200);
        app.update();
        let blink = app.world().get::<SegmentBlink>(slot).unwrap();
        assert_eq!(blink.follow_duration, Duration::from_millis(200));

        // Finish the 50 ms empty phase; the follow phase now runs at the new
        // duration, so a second 60 ms frame stays mid-phase.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            60,
        )));
        app.update();
        assert_eq!(
            app.world().get::<Segment>(slot).unwrap().state,
            SegmentState::Follow
        );
        app.update();
        assert_eq!(
            app.world().get::<Segment>(slot).unwrap().state,
            SegmentState::Follow
        );
    }

    #[test]
    fn changing_slot_count_reconciles_children() {
        let mut app = test_app();
        let bar = app
            .world_mut()
            .spawn((SegmentedBar::new(4), CircularBarValue::new(1.0)))
            .id();
        app.update();
        app.update();
        assert_eq!(
            segment_states(&mut app, bar),
            vec![
                (0, SegmentState::Fill, false),
                (1, SegmentState::Fill, false),
                (2, SegmentState::Fill, false),
                (3, SegmentState::Fill, false),
            ]
        );

        // Growing spawns the missing slots; value 1.0 now spans all six.
        app.world_mut()
            .get_mut::<SegmentedBar>(bar)
            .unwrap()
            .slot_count = 6;
        app.update();
        app.update();
        assert_eq!(
            segment_states(&mut app, bar),
            vec![
                (0, SegmentState::Fill, false),
                (1, SegmentState::Fill, false),
                (2, SegmentState::Fill, false),
                (3, SegmentState::Fill, false),
                (4, SegmentState::Fill, false),
                (5, SegmentState::Fill, false),
            ]
        );

        // Shrinking despawns the extras.
        app.world_mut()
            .get_mut::<SegmentedBar>(bar)
            .unwrap()
            .slot_count = 2;
        app.update();
        app.update();
        assert_eq!(
            segment_states(&mut app, bar),
            vec![
                (0, SegmentState::Fill, false),
                (1, SegmentState::Fill, false),
            ]
        );
    }

    #[test]
    fn segmented_bar_requires_a_value() {
        let mut app = test_app();
        let bar = app.world_mut().spawn(SegmentedBar::new(3)).id();
        app.update();
        app.update();

        assert!(app.world().get::<CircularBarValue>(bar).is_some());
        assert_eq!(
            segment_states(&mut app, bar),
            vec![
                (0, SegmentState::Empty, false),
                (1, SegmentState::Empty, false),
                (2, SegmentState::Empty, false),
            ]
        );
    }
}
