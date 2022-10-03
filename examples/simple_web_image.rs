// Copyright 2020 The Druid Authors.

//! This showcase demonstrates how to use the image widget and is
//! propperties. You can change the parameters in the GUI to see how
//! everything behaves.

// On Windows platform, don't show a console when opening the app.
#![windows_subsystem = "windows"]

use masonry::widget::WebImage;
use masonry::{AppLauncher, WindowDesc};

pub fn main() {
    let image = WebImage::new(
        "https://raw.githubusercontent.com/linebender/druid/screenshots/images/small_banner.png"
            .into(),
    );

    let main_window = WindowDesc::new(image)
        .window_size((650., 450.))
        .title("Flex Container Options");

    AppLauncher::with_window(main_window)
        .log_to_console()
        .launch()
        .expect("Failed to launch application");
}