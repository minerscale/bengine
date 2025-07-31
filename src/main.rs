#![windows_subsystem = "windows"]
#![warn(clippy::pedantic, clippy::cargo)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::struct_field_names)]

use log::info;
use tracing_mutex::stdsync::Mutex;

mod audio;
mod clock;
mod egui_backend;
mod egui_sdl3_event;
mod event_loop;
mod game;
mod gltf;
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
    let window = {
        sdl_context
            .video()
            .unwrap()
            .window("bengine", WIDTH, HEIGHT)
            .vulkan()
            .position_centered()
            .resizable()
            .build()
            .unwrap()
    };
    sdl_context.mouse().set_relative_mouse_mode(&window, true);

    let mut gfx = Renderer::new(WIDTH, HEIGHT, &window, &DESCRIPTOR_SET_LAYOUTS, &PIPELINES);

    let game = Mutex::new(Game::new(&gfx));

    let mut event_loop = EventLoop::new(sdl_context);

    info!("finished loading");

    event_loop.run(
        |input| {
            let extent = gfx.swapchain.images[0].extent;

            let mut minput = input.lock().unwrap();

            let framebuffer_resized = if let Some(framebuffer_size) = minput.framebuffer_resized {
                gfx.window_size = framebuffer_size;
                true
            } else {
                false
            };

            minput.framebuffer_resized = None;

            drop(minput);

            let mut mgame = game.lock().unwrap();
            mgame.gui.free_textures();
            mgame.gui.run();
            mgame.gui.update_textures(&gfx);
            mgame.gui.upload_clipped_primitives(&gfx);
            drop(mgame);

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
                        extent,
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
