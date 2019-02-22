use easycurses::*;

use ordered_float::NotNan;

mod prelude;

use crate::prelude::*;

use std::cmp::{max, min};
use std::thread::sleep;
use std::time::{Duration, Instant};

pub fn rotate_2d(pos: (f32, f32), rad: f32) -> (f32, f32) {
    let (x, y) = pos;
    let (s, c) = (rad.sin(), rad.cos());
    (x * c - y * s, y * c + x * s)
}

pub struct Camera {
    pub pos: (f32, f32, f32),
    pub rot: (f32, f32),
}

impl Camera {
    pub fn new(pos: (f32, f32, f32), rot: (f32, f32)) -> Camera {
        Camera { pos, rot }
    }

    pub fn update(&mut self, _: &mut EasyCurses, delta: f32, key: Option<Input>) {
        let s = delta * 10.;

        if let Some(input) = key {
            match input {
                Input::Character('q') => self.pos.1 += s,
                Input::Character('e') => self.pos.1 -= s,

                Input::Character('w')
                | Input::Character('a')
                | Input::Character('s')
                | Input::Character('d') => {
                    let (x, y) = (s * self.rot.1.sin(), s * self.rot.1.cos());
                    match input {
                        Input::Character('w') => {
                            self.pos.0 += x;
                            self.pos.2 += y;
                        }
                        Input::Character('s') => {
                            self.pos.0 -= x;
                            self.pos.2 -= y;
                        }
                        Input::Character('a') => {
                            self.pos.0 -= y;
                            self.pos.2 += x;
                        }
                        Input::Character('d') => {
                            self.pos.0 += y;
                            self.pos.2 -= x;
                        }
                        _ => unreachable!(),
                    }
                }

                Input::KeyUp => self.rot.0 -= s,
                Input::KeyDown => self.rot.0 += s,
                Input::KeyLeft => self.rot.1 -= s,
                Input::KeyRight => self.rot.1 += s,

                _ => {}
            }
            // println!("Input: {:?}", input);
        }
    }
}

pub trait Game {
    fn start(&mut self, term: &mut Term3D);
    fn update(&mut self, term: &mut Term3D, delta: f32);
}

pub struct Term3D {
    pub backend: EasyCurses,
    pub cam: Camera,
}

impl Term3D {
    pub fn new() -> Self {
        Self { backend: EasyCurses::initialize_system().unwrap(), cam: Camera::new((0., 0., 0.), (0., 0.))}
    }

    pub fn run<T: Game>(&mut self, game: &mut T) {
        self.backend.set_input_mode(InputMode::Character);
        self.backend.set_input_timeout(TimeoutMode::Immediate);
        self.backend.set_cursor_visibility(CursorVisibility::Invisible);
        self.backend.set_keypad_enabled(true);
        self.backend.set_echo(false);

        let frame_target_duration = Duration::new(1, 0).checked_div(60).unwrap();

        let verts = [
            (-1., -1., -1.),
            (1., -1., -1.),
            (1., 1., -1.),
            (-1., 1., -1.),
            (-1., -1., 1.),
            (1., -1., 1.),
            (1., 1., 1.),
            (-1., 1., 1.),
        ];
        // let edges = [
        //     (0., 1.),
        //     (1., 2.),
        //     (2., 3.),
        //     (3., 0.),
        //     (4., 5.),
        //     (5., 6.),
        //     (6., 7.),
        //     (7., 4.),
        //     (0., 4.),
        //     (1., 5.),
        //     (2., 6.),
        //     (3., 7.),
        // ];
        let faces = [
            [0., 1., 2., 3.],
            [4., 5., 6., 7.],
            [0., 1., 5., 4.],
            [2., 3., 7., 6.],
            [0., 3., 7., 4.],
            [1., 2., 6., 5.],
        ];

        const COLORS: &[Color] = &[
            Color::Red,
            Color::Green,
            Color::Blue,
            Color::Yellow,
            Color::White,
            Color::Magenta,
        ];

        let (mut h, mut w) = self.backend.get_row_col_count();
        let (mut cx, mut cy) = (w as f32 / 2., h as f32 / 2.);

        let mut delta_time: f32 = 0.;

        // Initialize game
        game.start(self);

        loop {
            let top_of_loop = Instant::now();

            let key = self.backend.get_input();
            if key == Some(Input::Character('\u{1b}')) {
                break;
            } else if key == Some(Input::KeyResize) {
                self.backend.resize(0, 0);
                let (height, width) = self.backend.get_row_col_count();
                w = width;
                h = height;
                cx = w as f32 / 2.;
                cy = h as f32 / 2.;
            } else {
                self.cam.update(&mut self.backend, delta_time, key);
            }

            game.update(self, delta_time);

            let after_updates = Instant::now();

            // clear screen
            self.backend.set_color_pair(ColorPair::default());
            for x in 0..w {
                for y in 0..h {
                    Self::draw_cell(&mut self.backend, ' ', x, y);
                }
            }

            let mut vert_list = Vec::<[f32; 3]>::new();
            let mut screen_coords = Vec::<IVec2>::new();

            for (x, y, z) in &verts {
                let (x, y, z) = (x - self.cam.pos.0, y - self.cam.pos.1, z - self.cam.pos.2);
                let (mut x, z) = rotate_2d((x, z), self.cam.rot.1);
                let (mut y, z) = rotate_2d((y, z), self.cam.rot.0);
                vert_list.push([x, y, z]);

                let f = 200. / z;
                x *= f;
                y *= f;
                screen_coords.push(IVec2::new((cx + x) as i32, (cy + y) as i32));
            }

            let mut face_list = Vec::<Vec<IVec2>>::new();
            let mut face_color = Vec::<Color>::new();
            let mut depth = Vec::<f32>::new();

            for i in 0..faces.len() {
                let face = faces[i];

                let mut on_screen = false;
                for &i in &face {
                    let p = screen_coords[i as usize];
                    if vert_list[i as usize][2] > 0. && p.x > 0 && p.x < w && p.y > 0 && p.y < h {
                        on_screen = true;
                        break;
                    }
                }

                if on_screen {
                    face_list.push(
                        face.iter()
                            .map(|&v| screen_coords[v as usize])
                            .collect(),
                    );
                    face_color.push(COLORS[i]);

                    // depth += [sum(sum(vert_list[j][k] for j in face)**2 for k in range(3))]
                    depth.push((0..3).map(|k| {
                        face.iter().map(|&j| {
                            vert_list[j as usize][k as usize]
                        }).sum::<f32>().powi(2)
                    }).sum::<f32>());
                }
            }

            let mut order = (0..face_list.len()).collect::<Vec<usize>>();
            order.sort_by_key(|&k| NotNan::new(depth[k]).unwrap());
            order.reverse();

            for i in order {
                Self::draw_tri(
                    &mut self.backend,
                    ColorPair::new(face_color[i], Color::Black),
                    face_list[i][0],
                    face_list[i][1],
                    face_list[i][2],
                );
                Self::draw_tri(
                    &mut self.backend,
                    ColorPair::new(face_color[i], Color::Black),
                    face_list[i][0],
                    face_list[i][3],
                    face_list[i][2],
                );
            }

            let elapsed_this_frame = top_of_loop.elapsed();
            if let Some(frame_remaining) = frame_target_duration.checked_sub(elapsed_this_frame) {
                sleep(frame_remaining);
            }

            self.backend.refresh();

            let elapsed_after_updates = after_updates.elapsed();
            delta_time = (elapsed_after_updates.as_secs() as f32)
                + ((elapsed_after_updates.subsec_nanos() as f32) / 1000000000.0);
        }
    }

    pub fn draw_cell(e: &mut EasyCurses, c: char, x: i32, y: i32) {
        // e.move_xy(x, y);
        e.move_rc(y, x);
        e.print_char(c);
    }

    fn draw_line_low(e: &mut EasyCurses, x0: i32, y0: i32, x1: i32, y1: i32) {
        let dx = x1 - x0;
        let mut dy = y1 - y0;
        let mut yi = 1;
        if dy < 0 {
            yi = -1;
            dy = -dy;
        }
        let mut d = 2 * dy - dx;
        let mut y = y0;

        e.set_color_pair(ColorPair::default());
        for x in x0..x1 {
            Term3D::draw_cell(e, '#', x, y);
            if d > 0 {
                y += yi;
                d -= 2 * dx;
            }
            d += 2 * dy;
        }
    }

    fn draw_line_high(e: &mut EasyCurses, x0: i32, y0: i32, x1: i32, y1: i32) {
        let mut dx = x1 - x0;
        let dy = y1 - y0;
        let mut xi = 1;
        if dx < 0 {
            xi = -1;
            dx = -dx;
        }
        let mut d = 2 * dx - dy;
        let mut x = x0;

        e.set_color_pair(ColorPair::default());
        for y in y0..y1 {
            Term3D::draw_cell(e, '#', x, y);
            if d > 0 {
                x += xi;
                d -= 2 * dy;
            }
            d += 2 * dx;
        }
    }

    pub fn draw_line(e: &mut EasyCurses, x0: i32, y0: i32, x1: i32, y1: i32) {
        if x0 == x1 {
            for y in y0..=y1 {
                Self::draw_cell(e, '|', x0, y);
            }
        } else if y0 == y1 {
            for x in x0..=x1 {
                Self::draw_cell(e, '-', x, y0);
            }
        } else {
            if (y1 - y0).abs() < (x1 - x0).abs() {
                if x0 > x1 {
                    Self::draw_line_low(e, x1, y1, x0, y0);
                } else {
                    Self::draw_line_low(e, x0, y0, x1, y1);
                }
            } else {
                if y0 > y1 {
                    Self::draw_line_high(e, x1, y1, x0, y0);
                } else {
                    Self::draw_line_high(e, x0, y0, x1, y1);
                }
            }
        }
    }

    /// # Returns
    /// (minimum x, maximum x, minimum y, maximum y)
    pub fn tri_bounding_box(v1: IVec2, v2: IVec2, v3: IVec2) -> (i32, i32, i32, i32) {
        let mut min_x = v1.x;
        let mut max_x = v1.x;
        let mut min_y = v1.y;
        let mut max_y = v1.y;

        for vec in &[v2, v3] {
            if vec.x < min_x {
                min_x = vec.x;
            } else if vec.x > max_x {
                max_x = vec.x;
            }

            if vec.y < min_y {
                min_y = vec.y;
            } else if vec.y > max_y {
                max_y = vec.y;
            }
        }

        (min_x, max_x, min_y, max_y)
    }

    pub fn draw_tri(e: &mut EasyCurses, color: ColorPair, v1: IVec2, v2: IVec2, v3: IVec2) {
        // calculate triangle bounding box
        let (minx, maxx, miny, maxy) = {
            let (minx, maxx, miny, maxy) = Self::tri_bounding_box(v1, v2, v3);
            // Clip box against render target bounds
            let (emax_y, emax_x) = e.get_row_col_count();
            (
                min(emax_x, max(0, minx)),
                min(emax_x, max(0, maxx)),
                min(emax_y, max(0, miny)),
                min(emax_y, max(0, maxy)),
            )
        };

        let vs1 = IVec2::new(v2.x - v1.x, v2.y - v1.y);
        let vs2 = IVec2::new(v3.x - v1.x, v3.y - v1.y);

        e.set_color_pair(color);
        for x in minx..=maxx {
            for y in miny..=maxy {
                let q = IVec2::new(x - v1.x, y - v1.y);

                let s = q.perp_dot_product(&vs2) / vs1.perp_dot_product(&vs2);
                let t = vs1.perp_dot_product(&q) / vs1.perp_dot_product(&vs2);

                if (s >= 0.) && (t >= 0.) && (s + t <= 1.) {
                    Self::draw_cell(e, '#', x, y);
                }
            }
        }
    }
}
