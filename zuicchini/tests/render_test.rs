use zuicchini::foundation::{Color, Image};
use zuicchini::render::{Painter, Stroke};

#[test]
fn paint_rect_fills_correct_pixels() {
    let mut img = Image::new(10, 10, 4);
    img.fill(Color::BLACK);

    {
        let mut p = Painter::new(&mut img);
        p.set_canvas_color(Color::BLACK);
        p.paint_rect(2.0, 3.0, 4.0, 2.0, Color::RED);
    }
    // Pixel inside the rect
    assert_eq!(img.pixel(3, 3), &[255, 0, 0, 255]);
    assert_eq!(img.pixel(5, 4), &[255, 0, 0, 255]);
    // Pixel outside the rect should be canvas color
    assert_eq!(img.pixel(0, 0), &[0, 0, 0, 255]);
    assert_eq!(img.pixel(7, 7), &[0, 0, 0, 255]);
}

#[test]
fn canvas_blend_works_in_painter() {
    let mut img = Image::new(4, 4, 4);
    img.fill(Color::rgb(100, 100, 100));

    {
        let mut p = Painter::new(&mut img);
        // Canvas = rgb(50,50,50), source = rgb(150,150,150), alpha = 128
        // target += (150 - 50) * 128 / 255 = target + 50 = 150
        p.set_canvas_color(Color::rgb(50, 50, 50));
        p.set_alpha(128);
        p.paint_rect(0.0, 0.0, 4.0, 4.0, Color::rgb(150, 150, 150));
    }
    let px = img.pixel(0, 0);
    assert_eq!(px[0], 150);
    assert_eq!(px[1], 150);
    assert_eq!(px[2], 150);
}

#[test]
fn clip_rect_respected() {
    let mut img = Image::new(10, 10, 4);
    img.fill(Color::BLACK);

    {
        let mut p = Painter::new(&mut img);
        p.set_canvas_color(Color::BLACK);
        p.clip_rect(2.0, 2.0, 4.0, 4.0);
        // Paint a rect that extends beyond the clip
        p.paint_rect(0.0, 0.0, 10.0, 10.0, Color::GREEN);
    }
    // Inside clip: should be painted
    assert_eq!(img.pixel(3, 3), &[0, 255, 0, 255]);
    // Outside clip: should be canvas color (untouched)
    assert_eq!(img.pixel(0, 0), &[0, 0, 0, 255]);
    assert_eq!(img.pixel(7, 7), &[0, 0, 0, 255]);
}

#[test]
fn coordinate_transforms() {
    let mut img = Image::new(20, 20, 4);
    img.fill(Color::BLACK);

    {
        let mut p = Painter::new(&mut img);
        p.set_canvas_color(Color::BLACK);
        p.translate(5.0, 5.0);
        p.paint_rect(0.0, 0.0, 2.0, 2.0, Color::BLUE);
    }
    // Translated rect should appear at (5,5)
    assert_eq!(img.pixel(5, 5), &[0, 0, 255, 255]);
    assert_eq!(img.pixel(6, 6), &[0, 0, 255, 255]);
    // Origin should be canvas color
    assert_eq!(img.pixel(0, 0), &[0, 0, 0, 255]);
}

#[test]
fn push_pop_state() {
    let mut img = Image::new(20, 20, 4);
    img.fill(Color::BLACK);

    {
        let mut p = Painter::new(&mut img);
        p.set_canvas_color(Color::BLACK);
        p.push_state();
        p.translate(10.0, 10.0);
        p.paint_rect(0.0, 0.0, 2.0, 2.0, Color::RED);
        p.pop_state();
        // After pop, translation is restored
        p.paint_rect(0.0, 0.0, 2.0, 2.0, Color::GREEN);
    }
    // Red at translated position
    assert_eq!(img.pixel(10, 10), &[255, 0, 0, 255]);
    // Green at origin (painted after pop) — canvas blended on top of red remnant
    assert_eq!(img.pixel(0, 0)[1], 255); // green channel
}

#[test]
fn paint_ellipse_basic() {
    let mut img = Image::new(20, 20, 4);
    img.fill(Color::BLACK);

    {
        let mut p = Painter::new(&mut img);
        p.set_canvas_color(Color::BLACK);
        p.paint_ellipse(10.0, 10.0, 5.0, 5.0, Color::RED);
    }
    // Center should be filled
    let px = img.pixel(10, 10);
    assert_eq!(px[0], 255); // red
                            // Far corner should be canvas color
    assert_eq!(img.pixel(0, 0), &[0, 0, 0, 255]);
}

#[test]
fn paint_line_basic() {
    let mut img = Image::new(10, 10, 4);
    img.fill(Color::BLACK);

    {
        let mut p = Painter::new(&mut img);
        p.set_canvas_color(Color::BLACK);
        p.paint_line(0.0, 0.0, 9.0, 0.0, Color::WHITE);
    }
    // Horizontal line at y=0
    assert_eq!(img.pixel(0, 0), &[255, 255, 255, 255]);
    assert_eq!(img.pixel(5, 0), &[255, 255, 255, 255]);
    // Below the line should be canvas color
    assert_eq!(img.pixel(0, 5), &[0, 0, 0, 255]);
}

#[test]
fn paint_rect_outlined() {
    let mut img = Image::new(20, 20, 4);
    img.fill(Color::BLACK);

    {
        let mut p = Painter::new(&mut img);
        p.set_canvas_color(Color::BLACK);
        let stroke = Stroke::new(Color::WHITE, 1.0);
        p.paint_rect_outlined(5.0, 5.0, 10.0, 10.0, &stroke);
    }
    // Top edge
    assert_eq!(img.pixel(5, 5), &[255, 255, 255, 255]);
    // Center should be canvas color (only outline)
    assert_eq!(img.pixel(10, 10), &[0, 0, 0, 255]);
}

#[test]
fn paint_image_colored_basic() {
    // Create a 2x2 greyscale image
    let mut alpha_img = Image::new(2, 2, 1);
    alpha_img.pixel_mut(0, 0)[0] = 255;
    alpha_img.pixel_mut(1, 0)[0] = 128;
    alpha_img.pixel_mut(0, 1)[0] = 0;
    alpha_img.pixel_mut(1, 1)[0] = 64;

    let mut target = Image::new(4, 4, 4);
    target.fill(Color::BLACK);

    {
        let mut p = Painter::new(&mut target);
        p.set_canvas_color(Color::BLACK);
        p.paint_image_colored(0.0, 0.0, 2.0, 2.0, &alpha_img, 0, 0, 2, 2, Color::RED);
    }
    // Top-left pixel: full red (alpha=255 from mask)
    let px = target.pixel(0, 0);
    assert_eq!(px[0], 255); // red
                            // Bottom-left pixel: no paint (alpha=0 from mask)
    let px2 = target.pixel(0, 1);
    assert_eq!(px2[0], 0); // still black
}
