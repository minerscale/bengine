#![feature(macro_metavar_expr_concat)]
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

    sdl_context.video().unwrap().text_input().start(&window);

    let mut gfx = Renderer::new(WIDTH, HEIGHT, &window, &DESCRIPTOR_SET_LAYOUTS, &PIPELINES);

    let game = Mutex::new(Game::new(&gfx));

    let mut event_loop = EventLoop::new(sdl_context, window);

    info!("finished loading");

    event_loop.run(
        |shared_state| {
            let mut mgame = game.lock().unwrap();
            let mut state = shared_state.lock().unwrap();

            let framebuffer_resized = if let Some(framebuffer_size) = state.framebuffer_resized {
                gfx.window_size = framebuffer_size;
                true
            } else {
                false
            };

            state.framebuffer_resized = None;

            mgame.gui.update(&gfx, &mut state);
            drop(state);
            drop(mgame);

            gfx.acquire_next_image(framebuffer_resized);
            gfx.draw(
                |device, render_pass, command_buffer, uniform_buffers, image| {
                    game.lock().unwrap().draw(
                        &mut shared_state.lock().unwrap(),
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
        |shared_state, events, modifiers| {
            game.lock()
                .unwrap()
                .update(&mut shared_state.lock().unwrap(), events, modifiers);
        },
    );

    gfx.wait_idle();
}
