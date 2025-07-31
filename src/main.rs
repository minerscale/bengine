#![windows_subsystem = "windows"]
#![warn(clippy::pedantic, clippy::cargo)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::struct_field_names)]

use log::info;
use tracing_mutex::stdsync::Mutex;

mod audio;
mod clock;
mod event_loop;
mod game;
mod gltf;
mod gui;
mod mesh;
mod node;
mod physics;
mod player;
mod renderer;
mod scene;
mod shader_pipelines;
mod skybox;
mod vertex;

use event_loop::EventLoop;
use game::Game;
use renderer::{HEIGHT, Renderer, WIDTH};
use shader_pipelines::{DESCRIPTOR_SET_LAYOUTS, PIPELINES};

pub const FOV: f32 = 100.0;

fn main() {
    env_logger::init();

    let sdl_context = sdl3::init().unwrap();
    let video = sdl_context.video().unwrap();

    let window = video
        .window("bengine", WIDTH, HEIGHT)
        .vulkan()
        .position_centered()
        .resizable()
        .build()
        .unwrap();
    sdl_context.mouse().set_relative_mouse_mode(&window, true);
    sdl_context.video().unwrap().text_input().start(&window);

    let mut gfx = Renderer::new(WIDTH, HEIGHT, &window, &DESCRIPTOR_SET_LAYOUTS, &PIPELINES);

    let game = Mutex::new(Game::new(&gfx));

    let mut event_loop = EventLoop::new(sdl_context);

    info!("finished loading");

    event_loop.run(
        |input| {
            let mut minput = input.lock().unwrap();

            let framebuffer_resized = if let Some(framebuffer_size) = minput.framebuffer_resized {
                gfx.window_size = framebuffer_size;
                true
            } else {
                false
            };

            minput.framebuffer_resized = None;

            let gui_scale = minput.gui_scale;
            drop(minput);

            game.lock().unwrap().gui.update(&gfx, gui_scale);

            gfx.acquire_next_image(framebuffer_resized);
            gfx.draw(
                |device, render_pass, command_buffer, uniform_buffers, image| {
                    game.lock().unwrap().draw(
                        input,
                        device,
                        render_pass,
                        command_buffer,
                        uniform_buffers,
                        image,
                    )
                },
            );
            gfx.present();
        },
        |input, events, modifiers| {
            game.lock().unwrap().update(input, events, modifiers);
        },
    );

    gfx.wait_idle();
}
