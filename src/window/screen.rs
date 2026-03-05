/// Information about a physical monitor.
#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub name: Option<String>,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub scale_factor: f64,
    pub primary: bool,
}

/// Tracks available monitors and virtual desktop bounds.
pub struct Screen {
    monitors: Vec<MonitorInfo>,
    /// Virtual desktop bounding box (x, y, w, h).
    pub virtual_bounds: (i32, i32, u32, u32),
}

impl Screen {
    /// Populate from winit's available monitors.
    pub fn from_event_loop(event_loop: &winit::event_loop::ActiveEventLoop) -> Self {
        let mut monitors = Vec::new();
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        let mut first = true;
        for handle in event_loop.available_monitors() {
            let pos = handle.position();
            let size = handle.size();
            let scale = handle.scale_factor();
            let name = handle.name();

            let info = MonitorInfo {
                name,
                position: (pos.x, pos.y),
                size: (size.width, size.height),
                scale_factor: scale,
                primary: first,
            };
            first = false;

            min_x = min_x.min(pos.x);
            min_y = min_y.min(pos.y);
            max_x = max_x.max(pos.x + size.width as i32);
            max_y = max_y.max(pos.y + size.height as i32);

            monitors.push(info);
        }

        let virtual_bounds = if monitors.is_empty() {
            (0, 0, 1920, 1080)
        } else {
            (min_x, min_y, (max_x - min_x) as u32, (max_y - min_y) as u32)
        };

        Self {
            monitors,
            virtual_bounds,
        }
    }

    pub fn monitors(&self) -> &[MonitorInfo] {
        &self.monitors
    }

    pub fn primary(&self) -> Option<&MonitorInfo> {
        self.monitors.iter().find(|m| m.primary)
    }
}
