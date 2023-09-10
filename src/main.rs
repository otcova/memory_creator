use image::{imageops, DynamicImage, Rgb};
use rayon::prelude::*;
use std::fs;
use std::io::{self, Write};
use std::sync::atomic::AtomicU32;
use std::sync::Mutex;

const PIXELS_PER_MM: u32 = 12;
const DINA4_WIDTH: u32 = 210 * PIXELS_PER_MM;
const DINA4_HEIGHT: u32 = 297 * PIXELS_PER_MM;

fn load_image_from_file<P: AsRef<std::path::Path>>(path: P) -> Option<DynamicImage> {
    image::io::Reader::open(path).ok()?.decode().ok()
}

fn read_u32(prompt: &str) -> u32 {
    loop {
        let mut input_text = String::new();

        print!("{}: ", prompt);
        io::stdout().flush().unwrap();

        io::stdin()
            .read_line(&mut input_text)
            .expect("failed to read from stdin");

        let trimmed = input_text.trim();
        match trimmed.parse::<u32>() {
            Ok(i) => return i,
            Err(..) => println!("Invalid input '{}'. Expected a positive integer.", trimmed),
        };
    }
}

fn main() {
    let Some(dir) = std::env::args().last() else {
        println!("No input folder");
        return;
    };

    let mut work_path = fs::canonicalize(dir).unwrap();

    if work_path.metadata().unwrap().is_file() {
        work_path.pop();
    }

    println!("Reading from folder: {}", work_path.display());

    let in_w = read_u32("Card width (mm)");
    let in_h = read_u32("Card height (mm)");

    let min_margin = 3 * PIXELS_PER_MM;
    let card_width = u32::max(min_margin * 2, in_w * PIXELS_PER_MM) - min_margin * 2;
    let card_height = u32::max(min_margin * 2, in_h * PIXELS_PER_MM) - min_margin * 2;

    let columns = DINA4_WIDTH / (min_margin * 2 + card_width);
    let rows = DINA4_WIDTH / (min_margin * 2 + card_height);
    let cards_per_page = columns * rows;
    let x_margin = (DINA4_WIDTH / columns - card_width) / 2;
    let y_margin = (DINA4_HEIGHT / rows - card_height) / 2;

    let pages = Mutex::new(vec![]);
    let pages_ref = &pages;

    let counter = AtomicU32::new(0);

    let iter = fs::read_dir(&work_path).unwrap().par_bridge();
    iter.for_each(move |entry| {
        let Ok(entry) = entry else {
            return;
        };
        let path = entry.path();

        // Load Image
        let Some(img) = load_image_from_file(&path) else {
            return;
        };

        let img = img.resize(card_width, card_height, imageops::FilterType::Triangle);
        let img = img.into_rgb8();

        let name = path.file_name().unwrap().to_string_lossy();
        println!(" > {}", &name);

        // Process Image

        let card_index = 2 * counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        for card_index in card_index..=card_index + 1 {
            let pos_index = card_index % cards_per_page;
            let x = x_margin + (pos_index % columns) * DINA4_WIDTH / columns;
            let y = y_margin + (pos_index / columns) * DINA4_HEIGHT / rows;
            let page_index = card_index / cards_per_page;

            let mut pages = pages_ref.lock().unwrap();
            while !(pages.len() as u32 > page_index) {
                let page =
                    image::RgbImage::from_pixel(DINA4_WIDTH, DINA4_HEIGHT, Rgb([255, 255, 255]));
                pages.push(page);
            }

            let page = &mut pages[page_index as usize];
            imageops::overlay(page, &img, x as i64, y as i64);
        }
    });

    // Save pages
    let save_path = work_path.join("memory");
    fs::create_dir_all(&save_path).unwrap();
    for entry in fs::read_dir(&save_path).unwrap() {
        let Ok(entry) = entry else {
            return;
        };
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        if file_name.starts_with("memory-page-") && file_name.ends_with(".jpg") {
            let _ = fs::remove_file(entry.path());
        }
    }

    let pages = pages.lock().unwrap();
    pages.par_iter().enumerate().for_each(|(i, page)| {
        let file_path = save_path.join(&format!("memory-page-{}.jpg", i));
        page.save(file_path).unwrap();
    });
}
