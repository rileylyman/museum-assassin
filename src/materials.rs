use macroquad::prelude::*;
use miniquad::{BlendFactor, BlendState, BlendValue, Equation};

pub fn shadow_postprocess() -> Material {
    let vertex = r#"
        #version 100
        attribute vec3 position;
        attribute vec2 texcoord;
        attribute vec4 color0;

        varying lowp vec2 uv;
        varying lowp vec4 color;

        uniform mat4 Model;
        uniform mat4 Projection;

        void main() {
            gl_Position = Projection * Model * vec4(position, 1);
            color = color0 / 255.0;
            uv = texcoord;
        }
    "#;

    let fragment = r#"
        #version 100
        precision lowp float;

        varying vec4 color;
        varying vec2 uv;

        uniform sampler2D VisibleTexture;
        uniform sampler2D ConesTexture;
        uniform vec4 BgColor;
        uniform float AlarmTime;
        uniform float SwipeT;

        void main() {
            if (SwipeT > -1.0 && SwipeT < 0.0 && uv.x > -SwipeT) {
                gl_FragColor = vec4(0, 0, 0, 1);
                return;
            }

            if (SwipeT >= 0.0 && SwipeT <= 1.0 && uv.x <= 1.0 - SwipeT) {
                gl_FragColor = vec4(0, 0, 0, 1);
                return;
            }

            vec4 vis = texture2D(VisibleTexture, uv);
            vec4 cones = texture2D(ConesTexture, uv);

            if (cones.a == 0.0) {
                vis = vis * 0.8;
            } else {
                if (cones.r > 0.0) {
                    vis = mix(vis, vec4(1, 0, 0, 1), 0.5);
                } else {
                    vis = vis;
                }
            }

            float alarmIntensity = cos(5.0 * AlarmTime - 3.141592) + 1.0;

            if (vis.a == 0.0) {
                gl_FragColor = BgColor;
            } else {
                gl_FragColor = mix(vis, vec4(1, 0, 0, 1), alarmIntensity * 0.1);
            }
        }
    "#;

    load_material(
        ShaderSource::Glsl { vertex, fragment },
        MaterialParams {
            pipeline_params: PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
            uniforms: vec![
                UniformDesc::new("AlarmTime", UniformType::Float1),
                UniformDesc::new("BgColor", UniformType::Float4),
                UniformDesc::new("SwipeT", UniformType::Float1),
            ],
            textures: vec!["VisibleTexture".into(), "ConesTexture".into()],
            ..Default::default()
        },
    )
    .unwrap()
}
