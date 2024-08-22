use std::sync::Arc;
use winit::window::Window;

pub struct State<'window> {
  surface: wgpu::Surface<'window>,
  device: wgpu::Device,
  queue: wgpu::Queue,
  config: wgpu::SurfaceConfiguration,
  size: winit::dpi::PhysicalSize<u32>,
  render_pipeline: wgpu::RenderPipeline,
}

impl<'window> State<'window> {
  // Creating some of the wgpu types requires async code
  pub fn new(window: Arc<Window>) -> State<'window> {
    pollster::block_on(State::new_async(window))
  }

  pub async fn new_async(window: Arc<Window>) -> State<'window> {
    let size = window.inner_size();
    // The instance is a handle to our GPU
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
      #[cfg(not(target_arch="wasm32"))]
      backends: wgpu::Backends::PRIMARY,
      #[cfg(target_arch="wasm32")]
      backends: wgpu::Backends::GL,
      ..Default::default()
    });

    let surface = instance.create_surface(Arc::clone(&window)).unwrap();
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
      power_preference: wgpu::PowerPreference::default(),
      force_fallback_adapter: false,
      // Request an adapter which can render to our surface
      compatible_surface: Some(&surface),
    })
    .await
    .expect("Failed to find an appropriate adapter");

    let (device, queue) = adapter.request_device(
      &wgpu::DeviceDescriptor {
        label: Some("Device Setup"),
        memory_hints: wgpu::MemoryHints::default(),
        required_features: wgpu::Features::empty(),
        // WebGL doesn't support all of wgpu's features, so if
        // we're building for the web, we'll have to disable some.
        // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
        required_limits: if cfg!(target_arch = "wasm32") {
          wgpu::Limits::downlevel_webgl2_defaults()
        } else {
          wgpu::Limits::default() //:downlevel_webgl2_defaults()
        },
      },
      None,
    ).await.unwrap();

    let surface_caps = surface.get_capabilities(&adapter);
    // Shader code in this tutorial assumes an sRGB surface texture. Using a different
    // one will result in all the colors coming out darker. If you want to support non
    // sRGB surfaces, you'll need to account for that when drawing to the frame.
    let surface_format = surface_caps.formats.iter()
    .find(|f| f.is_srgb())
    .copied()
    .unwrap_or(surface_caps.formats[0]);

    let config = wgpu::SurfaceConfiguration {
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
      format: surface_format,
      width: size.width,
      height: size.height,
      present_mode: wgpu::PresentMode::Fifo, // surface_caps.present_modes[0],
      alpha_mode: surface_caps.alpha_modes[0],
      view_formats: vec![],
      desired_maximum_frame_latency: 2,
    };

    let render_pipeline = State::render_pipeline(&device, &config);

    Self {
      surface,
      device,
      queue,
      config,
      size,
      render_pipeline,
      // window: &window,
    }
  }

  // pub fn window(&self) -> &Window {
  //   &self.window
  // }

  // pub fn input(&mut self, _event: &WindowEvent) -> bool {
  //   false
  // }

  pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
    if new_size.width > 0 && new_size.height > 0 {
      self.size = new_size;
      self.config.width = new_size.width;
      self.config.height = new_size.height;
      self.surface.configure(&self.device, &self.config);
    }
  }

  // draw
  pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
    let output = self.surface.get_current_texture().expect("Failed to acquire texture");

    // create texture_view with default settings
    let texture_view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

    // create command encoder for commands sent to wgpu
    let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
      label: Some("Render Encoder"),
    });

    {
      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Render Pass"),
        color_attachments: &[
          // This is what @location(0) in the fragment shader targets
          Some(wgpu::RenderPassColorAttachment {
            view: &texture_view,
            resolve_target: None,
            ops: wgpu::Operations {
              load: wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0,
              }),
              store: wgpu::StoreOp::Store,
            },
          })
        ],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
      });

      render_pass.set_pipeline(&self.render_pipeline);
      render_pass.draw(0..3, 0..1);
    }

    // submit will accept anything that implements IntoIter
    self.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
  }

  fn render_pipeline(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("Shader"),
      source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label: Some("Render Pipeline Layout"),
      bind_group_layouts: &[],
      push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("Render Pipeline"),
      layout: Some(&render_pipeline_layout),
      vertex: wgpu::VertexState {
        module: &shader,
        entry_point: "vs_main",
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        buffers: &[]
      },
      fragment: Some(wgpu::FragmentState {
        module: &shader,
        entry_point: "fs_main",
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        targets: &[Some(wgpu::ColorTargetState {
          format: config.format,
          blend: Some(wgpu::BlendState::REPLACE),
          write_mask: wgpu::ColorWrites::ALL,
        })],
      }),
      // field describes how to interpret our vertices when converting them into triangles.
      primitive: wgpu::PrimitiveState {
        // means that every three vertices will correspond to one triangle
        topology: wgpu::PrimitiveTopology::TriangleList,
        strip_index_format: None,
        // fields tell wgpu how to determine whether a given triangle is facing forward or not
        front_face: wgpu::FrontFace::Ccw, // triangle facing forward
        cull_mode: Some(wgpu::Face::Back), // Triangles that are not considered facing forward are culled (not included in the render)
        // Requires Features::DEPTH_CLIP_CONTROL
        unclipped_depth: false,
        // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
        polygon_mode: wgpu::PolygonMode::Fill,
        // Requires Features::CONSERVATIVE_RASTERIZATION
        conservative: false
      },
      depth_stencil: None,
      multisample: wgpu::MultisampleState {
        count: 1, // determines how many samples the pipeline will use (multisampling)
        mask: !0, // specifies which samples should be active. In this case, we are using all of them
        alpha_to_coverage_enabled: false // anti-aliasing
      },
      multiview: None, // indicates how many array layers the render attachments can have
      cache: None, // allows wgpu to cache shader compilation data. Only really useful for Android build targets.
    });

    return render_pipeline;
  }
}

// pub async fn run() -> Result<(), EventLoopError> {
//   cfg_if::cfg_if! {
//       if #[cfg(target_arch = "wasm32")] {
//           std::panic::set_hook(Box::new(console_error_panic_hook::hook));
//           console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");
//       } else {
//           env_logger::init();
//       }
//   }

//   let event_loop = EventLoop::new().unwrap();
//   event_loop.set_control_flow(ControlFlow::Poll);
//   let window = WindowBuilder::new().build(&event_loop).unwrap();


//   #[cfg(target_arch = "wasm32")]
//   {
//       // Winit prevents sizing with CSS, so we have to set
//       // the size manually when on the web.
//       use winit::dpi::PhysicalSize;
//       let _ = window.request_inner_size(PhysicalSize::new(450, 400));

//       use winit::platform::web::WindowExtWebSys;
//       web_sys::window()
//       .and_then(|win| win.document())
//       .and_then(|doc| {
//           let dst = doc.get_element_by_id("wasm-example")?;
//           let canvas = web_sys::Element::from(window.canvas()?);
//           dst.append_child(&canvas).ok()?;
//           Some(())
//       })
//       .expect("Couldn't append canvas to document body.");
//   }

//   let mut state = State::new(&window);
//   let mut surface_configured = false;

//   event_loop.run_app(&mut state)
// }
