
pub mod gui;

pub mod game_engine;
mod ai_logic;

pub mod include {
    pub use stb_image::image::{self, LoadResult};

    pub use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
    pub use fontdue::{Font, FontSettings};

    pub use std::ffi::CString;
    use std::collections::HashSet;
    pub use std::time::{SystemTime, UNIX_EPOCH, Duration, Instant};
    pub use core::{
        convert::{TryInto},
        mem::{size_of, size_of_val},
    };
    //use beryllium::events::SDL_SCANCODE_ESCAPE;
    pub use beryllium::{
        events::*,
        init::InitFlags,
        video::{CreateWinArgs, GlContextFlags, GlProfile, GlSwapInterval, GlWindow},
        Sdl,
    };
    pub use ogl33::*;
    pub use glam::*;
}

pub mod shaders{
    pub const VERT_SHADER: &str = r#"
    #version 330 core

    layout (location = 0) in vec3 pos;
    layout (location = 1) in vec2 uv;

    out vec2 TexCoord;
    uniform mat4 projection;
    uniform mat4 model;

    void main()
    {
    gl_Position = projection * model * vec4(pos, 1.0);
    TexCoord = uv;
    }
    "#;

    pub const FRAG_SHADER: &str = r#"
    #version 410 core

    in vec2 TexCoord;
    out vec4 final_color;

    uniform sampler2D u_tex;

    void main()
    {
        final_color = texture(u_tex, TexCoord);
    }
    "#;
    pub const TRANS_FRAG_SHADER: &str = r#"
    #version 410 core

    in vec2 TexCoord;
    out vec4 final_color;

    uniform sampler2D u_tex;

    void main()
    {
        float alpha = smoothstep(0.0,0.1, length(texColor.rgb));
        vec4 tex_color = texture(u_tex, TexCoord)
        final_color = vec4(tex_color.rgb,tex_color.a * alpha);
        if (final_color.a < 0.01) {
            discard;
        }
    }
    "#;

    pub const TEXT_VERT_SHADER: &str = r#"
    #version 410 core
    layout (location = 0) in vec2 aPos;
    layout (location = 1) in vec2 aTexCoord;

    out vec2 TexCoord;

    uniform mat4 projection;

    void main() {
        gl_Position = projection * vec4(aPos, 0.0, 1.0);
        TexCoord = aTexCoord;
    }
    "#;
    pub const TEXT_FRAG_SHADER: &str = r#"
    #version 410 core
    in vec2 TexCoord;
    out vec4 FragColor;

    uniform sampler2D text;
    uniform vec3 textColor;

    void main() {
        float alpha = texture(text, TexCoord).r;
        FragColor = vec4(textColor, alpha);
    }
    "#;
}

pub mod tools {
    pub use crate::include::*;
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::error::Error;
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use std::path::Path;

    // Vertex is a useful type for holding data for the main rectangle
    pub type Vertex = [f32; 5];
    //centered rectangle
    pub const C_RECTANGLE: [Vertex; 6] =
        [ // First triangle
            [-0.5, -0.5, 0.0, 0.0, 0.0],  // bottom-left
            [0.5, -0.5, 0.0, 1.0, 0.0],   // bottom-right
            [0.5, 0.5, 0.0, 1.0, 1.0],    // top-right

            // Second triangle
            [0.5, 0.5, 0.0, 1.0, 1.0],    // top-right
            [-0.5, 0.5, 0.0, 0.0, 1.0],   // top-left
            [-0.5, -0.5, 0.0, 0.0, 0.0],  // bottom-left
        ];

    //bottom left rectangle
    pub const BL_RECTANGLE: [Vertex; 6] =
        [ // First triangle
            [0.0, 0.0, 0.0, 0.0, 0.0],  // bottom-left
            [1.0, 0.0, 0.0, 1.0, 0.0],   // bottom-right
            [1.0, 1.0, 0.0, 1.0, 1.0],    // top-right

            // Second triangle
            [1.0, 1.0, 0.0, 1.0, 1.0],    // top-right
            [0.0, 1.0, 0.0, 0.0, 1.0],   // top-left
            [0.0, 0.0, 0.0, 0.0, 0.0],  // bottom-left
        ];
    pub static mut SCREEN_WIDTH: i32 = 800;
    pub static mut SCREEN_HEIGHT: i32 = 600;

    pub enum actions{
        NONE,
        MOVE {dir: directions},
    }

    pub enum directions{
        UP,
        DOWN,
        RIGHT,
        LEFT,
    }

    impl directions{
        pub const UP_VALUE:(i8,i8) = (0,1);
        pub const DOWN_VALUE:(i8,i8) = (0,-1);
        pub const RIGHT_VALUE:(i8,i8) = (1,0);
        pub const LEFT_VALUE:(i8,i8) = (-1,0);
        pub const fn value(&self) -> (i8,i8) {
            match self{
                directions::UP => Self::UP_VALUE,
                directions::DOWN => Self::DOWN_VALUE,
                directions::LEFT => Self::LEFT_VALUE,
                directions::RIGHT => Self::RIGHT_VALUE
            }
        }
    }

    pub fn ortho(width:i32, height:i32, mirror_vert:bool) -> Mat4{
        let mirror_vertical = glam::Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0));
        match mirror_vert {
            true => {return mirror_vertical * glam::Mat4::orthographic_rh_gl(0.0, width as f32, 0.0, height as f32, -1.0, 1.0);},
            false => {return glam::Mat4::orthographic_rh_gl(0.0, width as f32, 0.0, height as f32, -1.0, 1.0);}
        }

    }
    pub fn clear_color(r: f32, g: f32, b: f32, a: f32)
    {
        unsafe { glClearColor(r, g, b, a) }
    }

    pub fn clear() {
        unsafe {
            glClear(GL_COLOR_BUFFER_BIT);
        }
    }

    pub fn update(win_state: &mut bool, SDL: &Sdl, action: &mut actions, egui_input: &mut egui::RawInput, mouse_pos: &mut egui::Pos2) {
        while let Some((event, _timestamp)) = SDL.poll_events() {
            match event {
                Event::Quit => *win_state = false,
                Event::WindowResized {win_id, width, height } => {
                    unsafe {
                        glViewport(0, 0, width, height);

                        SCREEN_WIDTH = width;
                        SCREEN_HEIGHT = height;
                    }
                }
                Event::MouseMotion { x_win, y_win, .. } => {
                    *mouse_pos = egui::pos2(x_win as f32, y_win as f32);
                    egui_input.events.push(egui::Event::PointerMoved(*mouse_pos));
                }

                Event::MouseButton { button, pressed, .. } => {
                    let egui_button = match button {
                        1 => Some(egui::PointerButton::Primary),
                        3 => Some(egui::PointerButton::Secondary),
                        2 => Some(egui::PointerButton::Middle),
                        _ => None,
                    };
                    if let Some(egui_button) = egui_button {
                        egui_input.events.push(egui::Event::PointerButton {
                            pos: *mouse_pos,
                            button: egui_button,
                            pressed,
                            modifiers: egui::Modifiers::default(),
                        });
                    }
                }
                Event::Key { pressed, scancode, modifiers, repeat, .. } => {
                    if pressed && repeat == 0 {
                        match scancode {
                            SDL_SCANCODE_ESCAPE => *win_state = false,
                            SDL_SCANCODE_W => *action = actions::MOVE {dir: directions::UP},
                            SDL_SCANCODE_S => *action = actions::MOVE {dir: directions::DOWN},
                            SDL_SCANCODE_A => *action = actions::MOVE {dir: directions::LEFT},
                            SDL_SCANCODE_D => *action = actions::MOVE {dir: directions::RIGHT},
                            _ => (),
                        }
                    }
                    let m = modifiers.0;
                    let egui_mods = egui::Modifiers {
                        alt:     m & (KMOD_LALT.0  | KMOD_RALT.0)  != 0,
                        ctrl:    m & (KMOD_LCTRL.0 | KMOD_RCTRL.0) != 0,
                        shift:   m & (KMOD_LSHIFT.0| KMOD_RSHIFT.0)!= 0,
                        mac_cmd: false,
                        command: m & (KMOD_LCTRL.0 | KMOD_RCTRL.0) != 0,
                    };
                    let egui_key = match scancode {
                        SDL_SCANCODE_BACKSPACE             => Some(egui::Key::Backspace),
                        SDL_SCANCODE_DELETE                => Some(egui::Key::Delete),
                        SDL_SCANCODE_RETURN
                        | SDL_SCANCODE_KP_ENTER            => Some(egui::Key::Enter),
                        SDL_SCANCODE_LEFT                  => Some(egui::Key::ArrowLeft),
                        SDL_SCANCODE_RIGHT                 => Some(egui::Key::ArrowRight),
                        SDL_SCANCODE_UP                    => Some(egui::Key::ArrowUp),
                        SDL_SCANCODE_DOWN                  => Some(egui::Key::ArrowDown),
                        SDL_SCANCODE_HOME                  => Some(egui::Key::Home),
                        SDL_SCANCODE_END                   => Some(egui::Key::End),
                        SDL_SCANCODE_TAB                   => Some(egui::Key::Tab),
                        SDL_SCANCODE_ESCAPE                => Some(egui::Key::Escape),
                        SDL_SCANCODE_A                     => Some(egui::Key::A),
                        SDL_SCANCODE_C                     => Some(egui::Key::C),
                        SDL_SCANCODE_V                     => Some(egui::Key::V),
                        SDL_SCANCODE_X                     => Some(egui::Key::X),
                        SDL_SCANCODE_Z                     => Some(egui::Key::Z),
                        _                                  => None,
                    };
                    if let Some(key) = egui_key {
                        egui_input.events.push(egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed,
                            repeat: repeat > 0,
                            modifiers: egui_mods,
                        });
                    }
                }
                Event::TextInput { text, .. } => {
                    egui_input.events.push(egui::Event::Text(text));
                }
                _ => (),
            }
        }
    }


    pub fn test_success(value: i32, shader: GLuint) {
        unsafe {
            if value == 0 {
                let mut v: Vec<u8> = Vec::with_capacity(1024);
                let mut log_len = 0_i32;
                glGetShaderInfoLog(
                    shader,
                    1024,
                    &mut log_len,
                    v.as_mut_ptr().cast(),
                );
                v.set_len(log_len.try_into().unwrap());
                panic!("Fragment Compile Error: {}", String::from_utf8_lossy(&v));
            }
        }
    }


    pub fn init_sdl() -> Sdl {
        let sdl = Sdl::init(InitFlags::EVERYTHING);
        sdl.set_gl_context_major_version(3).unwrap();
        sdl.set_gl_context_minor_version(3).unwrap();
        sdl.set_gl_profile(GlProfile::Core).unwrap();
        let flags = GlContextFlags::default();
        sdl.set_gl_context_flags(flags).unwrap();
        return sdl;
    }

    pub fn init_window(win_title: &str, SDL: &Sdl) -> GlWindow {
        let win = SDL
            .create_gl_window(CreateWinArgs {
                title: win_title,
                width: unsafe{SCREEN_WIDTH},
                height: unsafe{SCREEN_HEIGHT},
                resizable: true,
                ..Default::default()
            })
            .expect("couldn't make a window and context");
        unsafe {
            load_gl_with(|f_name| win.get_proc_address(f_name.cast()));
            glViewport(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT);
            stb_image::stb_image::stbi_set_flip_vertically_on_load(1);
        }
        return win;
    }
    pub fn load_image(path: &str) -> u32 {
        let width: i32;
        let height: i32;
        let data: Vec<u8>;
        match image::load(path) {
            LoadResult::ImageU8(img) => {
                width = img.width.try_into().unwrap();
                height = img.height.try_into().unwrap();
                data = img.data;
            }
            LoadResult::ImageF32(_) => {
                panic!("HDR images not supported");
            }
            LoadResult::Error(err) => {
                panic!("Failed to load image '{}': {}", path, err);
            }
        }
        let mut tex = 0;
        unsafe {
            glGenTextures(1, &mut tex);
            glBindTexture(GL_TEXTURE_2D, tex);

            glTexParameteri(
                GL_TEXTURE_2D,
                GL_TEXTURE_WRAP_S,
                GL_REPEAT as i32,
            );
            glTexParameteri(
                GL_TEXTURE_2D,
                GL_TEXTURE_WRAP_T,
                GL_REPEAT as i32,
            );
            glTexParameteri(
                GL_TEXTURE_2D,
                GL_TEXTURE_MIN_FILTER,
                GL_LINEAR as i32,
            );
            glTexParameteri(
                GL_TEXTURE_2D,
                GL_TEXTURE_MAG_FILTER,
                GL_LINEAR as i32,
            );

            glTexImage2D(
                GL_TEXTURE_2D,
                0,
                GL_RGBA as i32,
                width,
                height,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                data.as_ptr().cast(),
            );

            glGenerateMipmap(GL_TEXTURE_2D);
        }
        return tex;
    }

    pub const MAP_PHYSICS_DELIMITER: &str = "---PHYSICS---";

    pub fn load_map(path: &str) -> Vec<Vec<i32>> {
        let file = File::open(Path::new(path)).unwrap();
        let reader = BufReader::new(file);
        let mut all_numbers: Vec<Vec<i32>> = Vec::new();
        for line_result in reader.lines() {
            let line = line_result.unwrap();
            if line.trim() == MAP_PHYSICS_DELIMITER { break; }
            let numbers_in_line: Result<Vec<i32>, _> = line
                .split_whitespace()
                .map(|s| s.parse())
                .collect();
            all_numbers.push(numbers_in_line.unwrap());
        }
        all_numbers.reverse();
        return all_numbers;
    }

    /// Returns a row-major grid of solid flags matching the tile map layout.
    /// Row 0 = world bottom row (same convention as load_map).
    /// Returns an empty Vec if no physics section exists in the file.
    pub fn load_physics(path: &str) -> Vec<Vec<bool>> {
        let file = File::open(Path::new(path)).unwrap();
        let reader = BufReader::new(file);
        let mut past_delimiter = false;
        let mut result: Vec<Vec<bool>> = Vec::new();
        for line_result in reader.lines() {
            let line = line_result.unwrap();
            if !past_delimiter {
                if line.trim() == MAP_PHYSICS_DELIMITER { past_delimiter = true; }
                continue;
            }
            let row: Vec<bool> = line
                .split_whitespace()
                .map(|s| s.parse::<i32>().unwrap_or(0) != 0)
                .collect();
            if !row.is_empty() { result.push(row); }
        }
        result.reverse(); // same row-ordering as load_map
        result
    }
    pub fn load_textures(path: &str) -> HashMap<i32, String> {
        let file_path = Path::new(&path);
        let file = File::open(file_path).unwrap();
        let reader = BufReader::new(file);
        let mut textures: HashMap<i32, String> = HashMap::new();

        for line_result in reader.lines() {
            let line = line_result.unwrap();
            let line_data: Vec<&str> = line.split_whitespace().collect();

            // Skip empty lines or comments
            if line_data.is_empty() || line_data[0].starts_with('#') {
                continue;
            }
            // Expecting format: "id texture_path"
            if line_data.len() >= 2 {
                if let Ok(id) = line_data[0].parse::<i32>() {
                    textures.insert(id, line_data[1].to_string());
                }
            }
        }
        return textures
    }
    pub fn bfs(x_tar: i32, y_tar: i32, x_start: i32, y_start: i32, current_map: &[Vec<i32>], ) -> Vec<(i32, i32)> {
        // Get map dimensions
        let height = current_map.len() as i32;
        let width = current_map.first().map_or(0, |row| row.len() as i32);

        // Check if start or target is invalid
        if x_start < 0 || y_start < 0 || y_start >= height || x_start >= width ||
            x_tar < 0 || y_tar < 0 || y_tar >= height || x_tar >= width ||
            current_map[y_start as usize][x_start as usize] != 0 ||
            current_map[y_tar as usize][x_tar as usize] != 0 {
            return Vec::new();
        }

        // BFS setup
        let mut queue = VecDeque::new();
        let mut parent = HashMap::new();
        let mut visited = HashSet::new();

        let start = (x_start, y_start);
        let target = (x_tar, y_tar);

        queue.push_back(start);
        visited.insert(start);
        parent.insert(start, None);

        // Directions: up, down, left, right
        const DIRECTIONS: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

        // BFS search
        while let Some((x, y)) = queue.pop_front() {
            if (x, y) == target {
                // Reconstruct path
                let mut path = Vec::new();
                let mut current = Some((x, y));

                while let Some(pos) = current {
                    path.push(pos);
                    current = parent[&pos];
                }

                path.reverse();
                return path;
            }

            // Check all 4 neighbors
            for (dx, dy) in DIRECTIONS {
                let nx = x + dx;
                let ny = y + dy;
                let next = (nx, ny);

                // Check if neighbor is valid and not visited
                if nx >= 0 && nx < width &&
                    ny >= 0 && ny < height &&
                    current_map[ny as usize][nx as usize] == 0 &&
                    !visited.contains(&next) {
                    queue.push_back(next);
                    visited.insert(next);
                    parent.insert(next, Some((x, y)));
                }
            }
        }

        Vec::new() // No path found
    }

    pub fn bfs_to_move(start_pos:(i32,i32), target_pos:(i32,i32), map: &[Vec<i32>]) -> Vec<(i32, i32)> {
        let working_vec:Vec<(i32,i32)> = bfs(start_pos.0, start_pos.1,target_pos.0, target_pos.1, map);
        let mut new_vec = working_vec.clone();
        for i in  0..working_vec.len() {
            new_vec[i] = (working_vec[i-1].0 - working_vec[i].0, working_vec[i-1].1 - working_vec[i].1);
        }
        return new_vec;
    }

    pub struct GLObject {
        vao: GLuint,
        vbo: GLuint,
        tex_id: u32,
        Shader: shader,

    }
    impl GLObject {
        pub fn new(vertex_data: [Vertex; 6], tex_path: &str, v_shader_path: &str, f_shader_path: &str) -> GLObject {
            let mut temp_object = GLObject {
                vao: 0,
                vbo: 0,
                tex_id: load_image(tex_path),
                Shader: shader::new(v_shader_path, f_shader_path)

            };
            unsafe {
                glGenVertexArrays(1, &mut temp_object.vao);
                assert_ne!(temp_object.vao, 0);
                glBindVertexArray(temp_object.vao);
                glGenBuffers(1, &mut temp_object.vbo);
                assert_ne!(temp_object.vbo, 0);

                glBindBuffer(GL_ARRAY_BUFFER, temp_object.vbo);
                glBufferData(GL_ARRAY_BUFFER, size_of_val(&vertex_data) as isize, vertex_data.as_ptr().cast(), GL_STATIC_DRAW);
                glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, size_of::<Vertex>().try_into().unwrap(), 0 as *const _);
                glEnableVertexAttribArray(0);
                glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE, size_of::<Vertex>().try_into().unwrap(), (3 * size_of::<f32>()) as *const _);
                glEnableVertexAttribArray(1);
                glBindVertexArray(0);
            }
            return temp_object;
        }
        pub fn bind(&self) {
            unsafe {
                glBindVertexArray(self.vao);
            }
        }
        pub fn unbind(&self) {
            unsafe {
                glBindVertexArray(0);
            }
        }
        pub fn draw(&self, x:i32, y:i32, scale:f32) {
            let projection = ortho(unsafe{SCREEN_WIDTH}, unsafe{SCREEN_HEIGHT}, false);
            //translate then scale
            let mut model = glam::Mat4::IDENTITY;
            model = model * Mat4::from_translation(vec3(x as f32, y as f32,0.0));
            model = model * Mat4::from_scale(vec3(scale, scale, 1.0));
            self.Shader.activate();
            self.bind();
            self.Shader.set_mat4(&projection, "projection");
            self.Shader.set_mat4(&model, "model");
            unsafe {
                glActiveTexture(GL_TEXTURE0);
                glBindTexture(GL_TEXTURE_2D, self.tex_id);
            }
            self.Shader.set_int(0, "u_tex");
            unsafe {
                glDrawArrays(GL_TRIANGLES, 0, 6);
            }
            self.unbind();
        }
    }

    pub struct TextRenderer {
        vao: GLuint,
        vbo: GLuint,
        Shader: shader,
        texture: GLuint,
        font: Font
    }
    impl TextRenderer {
        pub fn new(v_shader: &str, f_shader: &str, font_name: &str) -> Self {
            let font = Font::from_bytes(std::fs::read(font_name).expect("failed to read font"), FontSettings::default())
                .expect("Failed to load font");
            unsafe {
                // Create VAO and VBO
                let mut vao = 0;
                let mut vbo = 0;
                glGenVertexArrays(1, &mut vao);
                glGenBuffers(1, &mut vbo);

                glBindVertexArray(vao);
                glBindBuffer(GL_ARRAY_BUFFER, vbo);
                glBufferData(
                    GL_ARRAY_BUFFER,
                    (6 * 4 * size_of::<f32>()) as isize,
                    std::ptr::null(),
                    GL_DYNAMIC_DRAW,
                );

                // Position attribute
                glEnableVertexAttribArray(0);
                glVertexAttribPointer(
                    0,
                    2,
                    GL_FLOAT,
                    GL_FALSE,
                    (4 * size_of::<f32>()) as i32,
                    std::ptr::null(),
                );

                // Texture coordinate attribute
                glEnableVertexAttribArray(1);
                glVertexAttribPointer(
                    1,
                    2,
                    GL_FLOAT,
                    GL_FALSE,
                    (4 * size_of::<f32>()) as i32,
                    (2 * size_of::<f32>()) as *const _,
                );

                glBindBuffer(GL_ARRAY_BUFFER, 0);
                glBindVertexArray(0);

                // Create texture for glyph
                let mut texture = 0;
                glGenTextures(1, &mut texture);
                glBindTexture(GL_TEXTURE_2D, texture);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as i32);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as i32);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as i32);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
                glPixelStorei(GL_UNPACK_ALIGNMENT, 1);

                // Create shader program
                let Shader = shader::new(v_shader, f_shader);

                Self {
                    vao,
                    vbo,
                    Shader,
                    texture,
                    font
                }
            }
        }

        pub fn render_text(&self, text: &str, x: f32, y: f32, size: f32, color: Vec3) {
            unsafe {
                glEnable(GL_BLEND);
                glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
                self.Shader.activate();

                let projection = ortho(SCREEN_WIDTH,SCREEN_HEIGHT, true) ;
                self.Shader.set_mat4(&projection, "projection");
                self.Shader.set_vec3(color.x, color.y, color.z, "textColor");
                self.Shader.set_int(0, "text");

                glActiveTexture(GL_TEXTURE0);
                glBindVertexArray(self.vao);

                let mut cursor_x = x;
                let mut cursor_y = y;

                // Calculate baseline offset using a reference character
                let baseline_offset = {
                    let (metrics, _) = self.font.rasterize('A', size);
                    metrics.height as f32 + metrics.ymin as f32
                };

                for ch in text.chars() {
                    if ch == '\n' {
                        cursor_x = x;
                        cursor_y += size * 1.2;
                        continue;
                    }

                    let (metrics, bitmap) = self.font.rasterize(ch, size);

                    if metrics.width == 0 || metrics.height == 0 {
                        cursor_x += metrics.advance_width;
                        continue;
                    }

                    glBindTexture(GL_TEXTURE_2D, self.texture);
                    glTexImage2D(
                        GL_TEXTURE_2D,
                        0,
                        GL_R8 as i32,
                        metrics.width as i32,
                        metrics.height as i32,
                        0,
                        GL_RED,
                        GL_UNSIGNED_BYTE,
                        bitmap.as_ptr().cast(),
                    );

                    // Use baseline-relative positioning
                    let xpos = cursor_x + metrics.xmin as f32;
                    let ypos = cursor_y + baseline_offset - (metrics.height as f32 + metrics.ymin as f32);
                    let w = metrics.width as f32;
                    let h = metrics.height as f32;

                    #[rustfmt::skip]
                let vertices: [f32; 24] = [
                    xpos,     ypos + h, 0.0, 1.0,
                    xpos,     ypos,     0.0, 0.0,
                    xpos + w, ypos,     1.0, 0.0,

                    xpos,     ypos + h, 0.0, 1.0,
                    xpos + w, ypos,     1.0, 0.0,
                    xpos + w, ypos + h, 1.0, 1.0,
                ];

                    glBindBuffer(GL_ARRAY_BUFFER, self.vbo);
                    glBufferSubData(
                        GL_ARRAY_BUFFER,
                        0,
                        (vertices.len() * size_of::<f32>()) as isize,
                        vertices.as_ptr().cast(),
                    );
                    glBindBuffer(GL_ARRAY_BUFFER, 0);

                    glDrawArrays(GL_TRIANGLES, 0, 6);

                    cursor_x += metrics.advance_width;
                }

                glBindVertexArray(0);
                glBindTexture(GL_TEXTURE_2D, 0);
            }
        }
    }
    impl Drop for TextRenderer {
        fn drop(&mut self) {
            unsafe {
                glDeleteVertexArrays(1, &self.vao);
                glDeleteBuffers(1, &self.vbo);
                glDeleteTextures(1, &self.texture);
            }
        }
    }

    pub struct shader {
        shader_id: GLuint
    }
    impl shader {
        pub fn new(v_shader: &str, f_shader: &str) -> shader {
            unsafe {
                let vertex_shader = glCreateShader(GL_VERTEX_SHADER);
                assert_ne!(vertex_shader, 0);
                glShaderSource(vertex_shader, 1, &(v_shader.as_bytes().as_ptr().cast()), &(v_shader.len().try_into().unwrap()));
                glCompileShader(vertex_shader);
                let mut success = 0;
                glGetShaderiv(vertex_shader, GL_COMPILE_STATUS, &mut success);


                test_success(success, vertex_shader);


                let fragment_shader = glCreateShader(GL_FRAGMENT_SHADER);
                assert_ne!(fragment_shader, 0);
                glShaderSource(fragment_shader, 1, &(f_shader.as_bytes().as_ptr().cast()), &(f_shader.len().try_into().unwrap()));
                glCompileShader(fragment_shader);
                let mut success = 0;
                glGetShaderiv(fragment_shader, GL_COMPILE_STATUS, &mut success);


                test_success(success, fragment_shader);


                let mut shader_program = glCreateProgram();
                glAttachShader(shader_program, vertex_shader);
                glAttachShader(shader_program, fragment_shader);
                glLinkProgram(shader_program);
                let mut success = 0;
                glGetProgramiv(shader_program, GL_LINK_STATUS, &mut success);
                test_success(success, shader_program);

                glDeleteShader(vertex_shader);
                glDeleteShader(fragment_shader);

                shader {
                    shader_id: shader_program
                }
            }
        }
        pub fn activate(&self) {
            unsafe {
                glUseProgram(self.shader_id);
            }
        }
        fn get_uniform_location(&self, name: &str) -> i32 {
            let c_name = CString::new(name).expect("CString::new failed");
            unsafe {
                glGetUniformLocation(self.shader_id, c_name.as_ptr().cast())
            }
        }
        pub fn set_vec3(&self, x:f32, y:f32, z:f32, name:&str){
            unsafe {
                glUniform3f(self.get_uniform_location(name), x, y, z);
            }
        }
        pub fn set_int(&self, x:i32, name:&str){
            unsafe {
                glUniform1i(self.get_uniform_location(name), x);
            }
        }
        pub fn set_float(&self, x:f32, name:&str){
            unsafe {
                glUniform1f(self.get_uniform_location(name), x)
            }
        }
        pub fn set_mat4(&self, mat:&Mat4, name:&str){
            unsafe {
                glUniformMatrix4fv(self.get_uniform_location(name), 1, GL_FALSE, mat.to_cols_array().as_ptr().cast());
            }
        }

    }
    impl Drop for shader{
        fn drop(&mut self){
            unsafe{
                glDeleteProgram(self.shader_id);
            }
        }
    }
}