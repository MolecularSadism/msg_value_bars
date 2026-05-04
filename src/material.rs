use bevy::{
    asset::embedded_asset,
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct ValueBarMaterial {
    #[uniform(0)]
    pub uniforms: ValueBarUniforms,
}

#[derive(Clone, Copy, ShaderType)]
pub struct ValueBarUniforms {
    pub quad_px_size: Vec2,
    pub center_px: Vec2,

    pub frame_outer_radius: f32,
    pub frame_inner_radius: f32,
    pub frame_start_angle: f32,
    pub frame_end_angle: f32,

    pub lead_outer_radius: f32,
    pub lead_inner_radius: f32,
    pub lead_start_angle: f32,
    pub lead_end_angle: f32,

    pub fill_outer_radius: f32,
    pub fill_inner_radius: f32,
    pub fill_start_angle: f32,
    pub fill_end_angle: f32,

    pub frame_margin_outer_px: f32,
    pub frame_margin_inner_px: f32,
    pub frame_margin_angular_px: f32,
    pub _pad0: f32,

    pub fill_color: Vec4,
    pub follow_color: Vec4,
    pub frame_color: Vec4,
    pub background_color: Vec4,
}

impl Default for ValueBarUniforms {
    fn default() -> Self {
        let full = std::f32::consts::TAU;
        Self {
            quad_px_size: Vec2::splat(64.0),
            center_px: Vec2::splat(32.0),
            frame_outer_radius: 30.0,
            frame_inner_radius: 20.0,
            frame_start_angle: 0.0,
            frame_end_angle: full,
            lead_outer_radius: 30.0,
            lead_inner_radius: 20.0,
            lead_start_angle: 0.0,
            lead_end_angle: full,
            fill_outer_radius: 30.0,
            fill_inner_radius: 20.0,
            fill_start_angle: 0.0,
            fill_end_angle: full,
            frame_margin_outer_px: 1.0,
            frame_margin_inner_px: 1.0,
            frame_margin_angular_px: 1.0,
            _pad0: 0.0,
            fill_color: Vec4::new(0.30, 0.85, 0.35, 1.0),
            follow_color: Vec4::new(0.85, 0.20, 0.20, 1.0),
            frame_color: Vec4::new(0.05, 0.05, 0.07, 1.0),
            background_color: Vec4::ZERO,
        }
    }
}

impl UiMaterial for ValueBarMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://msg_value_bars/shaders/value_bar.wgsl".into()
    }
}

pub(crate) fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/value_bar.wgsl");
    app.add_plugins(UiMaterialPlugin::<ValueBarMaterial>::default());
}
