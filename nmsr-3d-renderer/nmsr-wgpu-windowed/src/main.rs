use std::borrow::Cow;
use std::{iter, mem};
use std::ptr::null;
use std::time::Instant;
use egui::{Context, FontDefinitions};
use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use renderdoc::OverlayBits;

use wgpu::{RenderPassDepthStencilAttachment, RequestAdapterOptions};
use wgpu::util::DeviceExt;
use winit::event;
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;
use nmsr_parts::high_level::camera::{Camera, CameraRotation};

use nmsr_parts::low_level::{Vec2, Vec3};
use nmsr_parts::low_level::cube::Cube;
use nmsr_parts::low_level::primitives::{PartPrimitive, Vertex};

#[tokio::main]
async fn main() {

    let mut renderdoc = renderdoc::RenderDoc::<renderdoc::V140>::new().expect("Failed to initialize RenderDoc");
    renderdoc.launch_replay_ui(true, None).expect("Failed to launch RenderDoc replay UI");

    let event_loop = EventLoop::new();
    let mut builder = winit::window::WindowBuilder::new();
    builder = builder.with_title("NMSR WGPU Windowed");
    let window = builder.build(&event_loop).unwrap();

    let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
    let dx12_shader_compiler = wgpu::util::dx12_shader_compiler_from_env().unwrap_or_default();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        dx12_shader_compiler,
    });

    let (size, surface) = unsafe {
        let size = window.inner_size();

        let surface = instance.create_surface(&window).unwrap();

        (size, surface)
    };

    let adapter = instance.request_adapter(&RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }).await.expect("Failed to find an appropriate adapter");


    let adapter_info = adapter.get_info();
    println!("Using {} ({:?})", adapter_info.name, adapter_info.backend);

    let (device, queue) = adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::default(),
        },
        None,
    ).await.expect("Unable to find a suitable GPU adapter!");

    let mut config = surface
        .get_default_config(&adapter, size.width, size.height)
        .expect("Surface isn't supported by the adapter.");
    let surface_view_format = config.format;
    config.view_formats.push(surface_view_format);
    surface.configure(&device, &config);

    let uv = Vec2::new(0.0, 0.0);
    let uv2 = Vec2::new(1.0, 1.0);

    let mut camera = Camera::new(Vec3::new(0.0, 4.0, -2.0), CameraRotation {
        yaw: 0.0,
        pitch: 0.0,
    }, 110f32);

    let to_render = //vec![
        Cube::new(Vec3::new(0.0, 4.0, 0.0), Vec3::new(1.0, 1.0, 1.0), [uv, uv2], [uv, uv2], [uv, uv2], [uv, uv2], [uv, uv2], [uv, uv2])
        //,Cube::new(Vec3::new(0.0, 4.5, 0.0), Vec3::new(0.5, 0.5, 0.5), [uv, uv2], [uv, uv2], [uv, uv2], [uv, uv2], [uv, uv2], [uv, uv2]),
   //]
    ;

    // Create the vertex and index buffers
    let vertex_size = mem::size_of::<Vertex>();
    let (vertex_data, index_data) = (to_render.get_vertices(), to_render.get_indices());

    let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(&vertex_data),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Index Buffer"),
        contents: bytemuck::cast_slice(&index_data),
        usage: wgpu::BufferUsages::INDEX,
    });

    // Create pipeline layout
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(64),
                },
                count: None,
            }
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });


    let mx_total = camera.generate_view_projection_matrix(config.width as f32 / config.height as f32);
    let mx_ref: &[f32; 16] = mx_total.as_ref();
    let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniform Buffer"),
        contents: bytemuck::cast_slice(mx_ref),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    // Create bind group
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            },
        ],
        label: None,
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });

    let vertex_buffers = [wgpu::VertexBufferLayout {
        array_stride: vertex_size as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 4 * 4,
                shader_location: 1,
            },
        ],
    }];

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &vertex_buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(config.view_formats[0].into())],
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            front_face: wgpu::FrontFace::Cw,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let mut egui_rpass = RenderPass::new(&device, surface_view_format, 1);

    let mut platform = Platform::new(PlatformDescriptor {
        physical_width: config.width as u32,
        physical_height: config.height as u32,
        scale_factor: window.scale_factor(),
        font_definitions: FontDefinitions::default(),
        style: Default::default(),
    });

    println!("Entering render loop...");
    let start_time = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        platform.handle_event(&event);

        match event {
            event::Event::RedrawEventsCleared => {
                window.request_redraw();
            }
            event::Event::WindowEvent {
                event:
                WindowEvent::Resized(size)
                | WindowEvent::ScaleFactorChanged {
                    new_inner_size: &mut size,
                    ..
                },
                ..
            } => {
                // Once winit is fixed, the detection conditions here can be removed.
                // https://github.com/rust-windowing/winit/issues/2876
                let max_dimension = adapter.limits().max_texture_dimension_2d;
                if size.width > max_dimension || size.height > max_dimension {
                    println!(
                        "The resizing size {:?} exceeds the limit of {}.",
                        size,
                        max_dimension
                    );
                } else {
                    println!("Resizing to {:?}", size);
                    config.width = size.width.max(1);
                    config.height = size.height.max(1);
                    surface.configure(&device, &config);
                }
            }
            event::Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = winit::event_loop::ControlFlow::Exit;
            },
            // On keyboard input, move the camera
            // W is forward, S is backward, A is left, D is right, Q is up, E is down
            // We are facing South
            event::Event::WindowEvent { event: WindowEvent::KeyboardInput { input, .. }, .. } => {
                let mut changed = false;
                if input.state == winit::event::ElementState::Pressed {
                    match input.virtual_keycode {
                        Some(winit::event::VirtualKeyCode::W) => {
                            camera.set_z(camera.position.z + 0.5);
                            changed = true;
                        },
                        Some(winit::event::VirtualKeyCode::S) => {
                            camera.set_z(camera.position.z - 0.5);
                            changed = true;
                        },
                        Some(winit::event::VirtualKeyCode::A) => {
                            camera.set_x(camera.position.x + 0.5);
                            changed = true;
                        },
                        Some(winit::event::VirtualKeyCode::D) => {
                            camera.set_x(camera.position.x - 0.5);
                            changed = true;
                        },
                        Some(winit::event::VirtualKeyCode::Q) => {
                            camera.set_y(camera.position.y + 0.5);
                            changed = true;
                        },
                        Some(winit::event::VirtualKeyCode::E) => {
                            camera.set_y(camera.position.y - 0.5);
                            changed = true;
                        },
                        // R
                        Some(winit::event::VirtualKeyCode::R) => {
                            println!("Triggering RenderDoc capture.");
                            renderdoc.trigger_capture();
                        },
                        _ => {},
                    }
                }
                if changed {
                    let mx_total = camera.generate_view_projection_matrix(config.width as f32 / config.height as f32);
                    let mx_ref: &[f32; 16] = mx_total.as_ref();
                    queue.write_buffer(&uniform_buf, 0, bytemuck::cast_slice(mx_ref));
                }
            },
            event::Event::RedrawRequested(_) => {
                platform.update_time(start_time.elapsed().as_secs_f64());

                let frame = match surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(_) => {
                        surface.configure(&device, &config);
                        surface
                            .get_current_texture()
                            .expect("Failed to acquire next surface texture!")
                    }
                };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
                    format: Some(surface_view_format),
                    ..wgpu::TextureViewDescriptor::default()
                });

                let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
                    size: wgpu::Extent3d {
                        width: config.width,
                        height: config.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth32Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    label: None,
                    view_formats: &[],
                });
                let depth = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

                device.push_error_scope(wgpu::ErrorFilter::Validation);

                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Main render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.2,
                                    b: 0.3,
                                    a: 1.0,
                                }),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                            view: &depth,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: true,
                            }),
                            stencil_ops: None,
                        }),
                    });

                    rpass.push_debug_group("Prepare data for draw.");
                    rpass.set_pipeline(&pipeline);
                    rpass.set_bind_group(0, &bind_group, &[]);
                    rpass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint16);
                    rpass.set_vertex_buffer(0, vertex_buf.slice(..));
                    rpass.pop_debug_group();
                    rpass.insert_debug_marker("Draw!");
                    rpass.draw_indexed(0..(index_data.len() as u32), 0, 0..1);
                }

                queue.submit(Some(encoder.finish()));

                // Begin to draw the UI frame.
                platform.begin_frame();

                // Draw the demo application.
                {
                    debug_ui(&platform.context(), &mut camera);
                }

                // End the UI frame. We could now handle the output and draw the UI with the backend.
                let full_output = platform.end_frame(Some(&window));
                let paint_jobs = platform.context().tessellate(full_output.shapes);

                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("encoder"),
                });

                // Upload all resources for the GPU.
                let screen_descriptor = ScreenDescriptor {
                    physical_width: config.width,
                    physical_height: config.height,
                    scale_factor: window.scale_factor() as f32,
                };
                let tdelta: egui::TexturesDelta = full_output.textures_delta;
                egui_rpass
                    .add_textures(&device, &queue, &tdelta)
                    .expect("add texture ok");
                egui_rpass.update_buffers(&device, &queue, &paint_jobs, &screen_descriptor);

                // Record all render passes.
                egui_rpass
                    .execute(
                        &mut encoder,
                        &view,
                        &paint_jobs,
                        &screen_descriptor,
                        None,
                    )
                    .unwrap();
                // Submit the commands.
                queue.submit(iter::once(encoder.finish()));

                egui_rpass
                    .remove_textures(tdelta)
                    .expect("remove texture ok");

                frame.present();

                let mx_total = camera.generate_view_projection_matrix(config.width as f32 / config.height as f32);
                let mx_ref: &[f32; 16] = mx_total.as_ref();
                queue.write_buffer(&uniform_buf, 0, bytemuck::cast_slice(mx_ref));
            }
            _ => {}
        }
    });
}

fn debug_ui(ctx: &Context, camera: &mut Camera) {
    egui::Window::new("Camera")
        .vscroll(true)
        .show(ctx, |ui| {
            ui.label("Camera");
            ui.label("X");
            ui.add(egui::DragValue::new(&mut camera.position.x));
            ui.label("Y");
            ui.add(egui::DragValue::new(&mut camera.position.y));
            ui.label("Z");
            ui.add(egui::DragValue::new(&mut camera.position.z));
            ui.label("Yaw");
            ui.add(egui::DragValue::new(&mut camera.rotation.yaw));
            ui.label("Pitch");
            ui.add(egui::DragValue::new(&mut camera.rotation.pitch));
            ui.label("Fov");
            ui.add(egui::DragValue::new(&mut camera.fov));
        });

}
