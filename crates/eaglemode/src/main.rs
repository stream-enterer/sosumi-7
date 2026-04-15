fn main() {
    // 1. Parse CLI args (simplified)
    let args: Vec<String> = std::env::args().collect();
    let mut fullscreen = false;
    let mut visit: Option<String> = None;
    let mut no_client = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-fullscreen" => fullscreen = true,
            "-noclient" => no_client = true,
            "-noserver" => { /* reserved for future IPC server disable */ }
            "-visit" => {
                i += 1;
                if i < args.len() {
                    visit = Some(args[i].clone());
                }
            }
            _ => {}
        }
        i += 1;
    }

    // 2. Try IPC client (unless -noclient)
    if !no_client {
        let server_name = emMain::emMain::CalcServerName();
        if emMain::emMain::try_ipc_client(&server_name, visit.as_deref()) {
            // Another instance handled the request; exit.
            return;
        }
    }

    // 3. Start GUI framework
    let config = emMain::emMainWindow::emMainWindowConfig {
        fullscreen,
        visit,
        ..Default::default()
    };

    let setup = Box::new(
        move |app: &mut emcore::emGUIFramework::App,
              event_loop: &winit::event_loop::ActiveEventLoop| {
            let mw = emMain::emMainWindow::create_main_window(app, event_loop, config);
            emMain::emMainWindow::set_main_window(mw);

            // Register per-frame callback to drive startup engine and main window cycle.
            app.set_frame_callback(Box::new(|app| {
                emMain::emMainWindow::with_main_window(|mw| {
                    mw.cycle_startup(app);
                });
            }));
        },
    );

    emcore::emGUIFramework::App::new(setup).run();
}
