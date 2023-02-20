use cgmath::{Quaternion, Rotation3};
use eframe::egui;
use encase::{ShaderType, UniformBuffer};
use wgpu::util::DeviceExt;

#[derive(Clone, Copy)]
struct Camera {
    position: cgmath::Vector3<f32>,
    rotation: Quaternion<f32>,
}

#[derive(Clone, Copy, ShaderType)]
struct CameraUniform {
    position: cgmath::Vector3<f32>,
    forward: cgmath::Vector3<f32>,
    right: cgmath::Vector3<f32>,
    up: cgmath::Vector3<f32>,
}

impl From<Camera> for CameraUniform {
    fn from(camera: Camera) -> Self {
        let forward = camera.rotation * cgmath::vec3(0.0, 0.0, 1.0);
        let right = camera.rotation * cgmath::vec3(1.0, 0.0, 0.0);
        let up = camera.rotation * cgmath::vec3(0.0, 1.0, 0.0);
        Self {
            position: camera.position,
            forward,
            right,
            up,
        }
    }
}

pub struct App {
    last_frame_time: std::time::Instant,
    fixed_update_time: f64, // change this to std::time::Duration at some point
    last_frame_update_duration: std::time::Duration,
    last_fixed_update_duration: std::time::Duration,
    counter: isize,
    texture_size: (usize, usize),
    texture_bind_group: wgpu::BindGroup,
    texture_id: egui::TextureId,
    pipeline: wgpu::ComputePipeline,
    camera: Camera,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let render_state = cc.wgpu_render_state.as_ref().unwrap();

        let (width, height) = (1usize, 1usize);
        let texture_size = wgpu::Extent3d {
            width: width as _,
            height: height as _,
            depth_or_array_layers: 1,
        };

        let texture = render_state
            .device
            .create_texture(&wgpu::TextureDescriptor {
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
                label: Some("texture"),
                view_formats: &[],
            });

        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &texture.create_view(&wgpu::TextureViewDescriptor {
                ..Default::default()
            }),
            wgpu::FilterMode::Linear,
        );

        let shader = render_state
            .device
            .create_shader_module(wgpu::include_wgsl!("./shader.wgsl"));

        let pipeline =
            render_state
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Pipeline"),
                    layout: None,
                    module: &shader,
                    entry_point: "main",
                });

        let texture_bind_group =
            render_state
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Texture bind group"),
                    layout: &pipeline.get_bind_group_layout(0),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    }],
                });

        let camera = Camera {
            position: (0.0, 0.0, 0.0).into(),
            rotation: Quaternion::from_axis_angle((0.0, 0.0, 1.0).into(), cgmath::Deg(0.0)),
        };

        let camera_buffer = {
            let camera_uniform: CameraUniform = camera.into();
            let mut buffer = UniformBuffer::new(
                [0u8; <CameraUniform as ShaderType>::METADATA.min_size().get() as _],
            );
            buffer.write(&camera_uniform).unwrap();
            render_state
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Camera Buffer"),
                    contents: &buffer.into_inner(),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                })
        };

        let camera_bind_group = render_state
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &pipeline.get_bind_group_layout(1),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }],
                label: Some("camera_bind_group"),
            });

        Self {
            last_frame_time: std::time::Instant::now(),
            fixed_update_time: 0.0,
            last_frame_update_duration: std::time::Duration::ZERO,
            last_fixed_update_duration: std::time::Duration::ZERO,
            counter: 0,
            texture_size: (width, height),
            texture_bind_group,
            texture_id,
            pipeline,
            camera,
            camera_buffer,
            camera_bind_group,
        }
    }

    const FIXED_UPDATE_TIMESTEP: f64 = 1.0 / 60.0;

    fn render(
        &mut self,
        _ts: f64,
        render_state: &egui_wgpu::RenderState,
        size @ (width, height): (usize, usize),
    ) {
        let start_frame_time = std::time::Instant::now();

        if self.texture_size != size && width != 0 && height != 0 {
            let mut renderer = render_state.renderer.write();
            renderer.free_texture(&self.texture_id);

            let texture_size = wgpu::Extent3d {
                width: width as _,
                height: height as _,
                depth_or_array_layers: 1,
            };

            let texture = render_state
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    size: texture_size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::STORAGE_BINDING,
                    label: Some("texture"),
                    view_formats: &[],
                });

            self.texture_id = renderer.register_native_texture(
                &render_state.device,
                &texture.create_view(&wgpu::TextureViewDescriptor {
                    ..Default::default()
                }),
                wgpu::FilterMode::Linear,
            );

            self.texture_bind_group =
                render_state
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Texture bind group"),
                        layout: &self.pipeline.get_bind_group_layout(0),
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                            ),
                        }],
                    });

            self.texture_size = size;
        }

        // Update camera uniform
        {
            let camera_uniform: CameraUniform = self.camera.into();
            let mut buffer = UniformBuffer::new(
                [0u8; <CameraUniform as ShaderType>::METADATA.min_size().get() as _],
            );
            buffer.write(&camera_uniform).unwrap();
            render_state
                .queue
                .write_buffer(&self.camera_buffer, 0, &buffer.into_inner());
        }

        let mut encoder = render_state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let workgroup_size = (16, 16);
            let (dispatch_with, dispatch_height) = (
                (self.texture_size.0 + workgroup_size.0 - 1) / workgroup_size.0,
                (self.texture_size.1 + workgroup_size.1 - 1) / workgroup_size.1,
            );
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute pass"),
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &self.texture_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            compute_pass.dispatch_workgroups(dispatch_with as _, dispatch_height as _, 1);
        }
        render_state.queue.submit([encoder.finish()]);

        self.last_frame_update_duration = start_frame_time.elapsed();
    }

    fn fixed_update(&mut self) {
        let start_fixed_update_time = std::time::Instant::now();

        // TODO: update stuff

        self.last_fixed_update_duration = start_fixed_update_time.elapsed();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let start_time = std::time::Instant::now();
        let dt = start_time.duration_since(self.last_frame_time);
        let ts = dt.as_secs_f64();

        self.fixed_update_time += ts;
        while self.fixed_update_time >= Self::FIXED_UPDATE_TIMESTEP {
            self.fixed_update();
            self.fixed_update_time -= Self::FIXED_UPDATE_TIMESTEP;
        }

        egui::SidePanel::left("Counting").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Count Up").clicked() {
                    self.counter += 1;
                }
                if ui.button("Count Down").clicked() {
                    self.counter -= 1;
                }
            });

            ui.label(format!("Current count: {}", self.counter));

            ui.label(format!("FPS: {:.3}", 1.0 / ts));
            ui.label(format!(
                "Render time: {:.3}ms",
                self.last_frame_update_duration.as_secs_f64() * 1000.0
            ));
            ui.label(format!(
                "Fixed update time: {:.3}ms",
                self.last_fixed_update_duration.as_secs_f64() * 1000.0
            ));

            ui.allocate_space(ui.available_size());
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let size = ui.available_size();
                self.render(
                    ts,
                    frame.wgpu_render_state().unwrap(),
                    (size.x as _, size.y as _),
                );
                ui.image(self.texture_id, size);
            });

        if !ctx.wants_pointer_input() {
            ctx.input(|i| {
                if i.pointer.secondary_down() {
                    let rotation_horizontal = cgmath::Quaternion::from_angle_y(cgmath::Deg(
                        i.pointer.velocity().x * ts as f32,
                    ));
                    let rotation_vertical = cgmath::Quaternion::from_angle_x(cgmath::Deg(
                        i.pointer.velocity().y * ts as f32,
                    ));
                    self.camera.rotation = self.camera.rotation * rotation_horizontal;
                    self.camera.rotation = self.camera.rotation * rotation_vertical;
                }
            });
        }

        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                let rotation_roll =
                    cgmath::Quaternion::from_angle_z(cgmath::Deg(if i.key_down(egui::Key::Q) {
                        90.0 * ts as f32
                    } else if i.key_down(egui::Key::E) {
                        -90.0 * ts as f32
                    } else {
                        0.0
                    }));
                self.camera.rotation = self.camera.rotation * rotation_roll;

                const CAMERA_SPEED: f32 = 2.0;

                let forward = self.camera.rotation * cgmath::vec3(0.0, 0.0, 1.0);
                let right = self.camera.rotation * cgmath::vec3(1.0, 0.0, 0.0);
                let up = self.camera.rotation * cgmath::vec3(0.0, 1.0, 0.0);

                if i.key_down(egui::Key::W) {
                    self.camera.position += CAMERA_SPEED * forward * ts as f32;
                }
                if i.key_down(egui::Key::S) {
                    self.camera.position -= CAMERA_SPEED * forward * ts as f32;
                }
                if i.key_down(egui::Key::A) {
                    self.camera.position -= CAMERA_SPEED * right * ts as f32;
                }
                if i.key_down(egui::Key::D) {
                    self.camera.position += CAMERA_SPEED * right * ts as f32;
                }
                if i.modifiers.ctrl {
                    self.camera.position -= CAMERA_SPEED * up * ts as f32;
                }
                if i.key_down(egui::Key::Space) {
                    self.camera.position += CAMERA_SPEED * up * ts as f32;
                }
            });
        }

        self.last_frame_time = start_time;
        ctx.request_repaint();
    }
}
