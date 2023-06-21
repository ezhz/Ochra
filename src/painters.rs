
use super::ogl::*;
use std::str::*;

// ----------------------------------------------------------------------------------------------------

pub struct Canvas
{
    pointers: FunctionPointers,
    program: Program,
    vao: VertexArrayObject,
    #[allow(dead_code)]
    vbo: Buffer
}

impl Canvas
{
    pub fn new
    (
        pointers: &FunctionPointers,
        fragment_code: &str
    ) -> Self
    {
        let program = link_program
        (
           pointers,
           &[
               &compile_shader
               (
                   &pointers,
                   VERTEX_SHADER,
                   &"
                   #version 100
                   attribute vec2 corner;
                   varying vec2 st;
                   void main()
                   {
                       gl_Position = vec4(corner, 0.0, 1.0);
                       st = corner * 0.5 + 0.5;
                   }
                   \0"
               ).unwrap(),
               &compile_shader
               (
                   &pointers,
                   FRAGMENT_SHADER,
                   &format!("{fragment_code}\0")
               ).unwrap()
           ]
        ).unwrap();
        let vao = VertexArrayObject::new(pointers);
        unsafe{pointers.BindVertexArray(*vao)}
        let corners = 
            [[-1.0, -1.0], [-1.0, 1.0], [1.0, 1.0], [1.0, -1.0]]
                .to_attribute
                (
                    pointers,
                    get_attribute_location
                    (
                        pointers, 
                        &program, 
                        &"corner"
                    ).unwrap()
                ).unwrap();
        Self
        {
            pointers: pointers.clone(),
            program,
            vao,
            vbo: corners
        }
    }
    
    fn set_uniform<T>(&self, name: &str, value: T) -> ()
    where
        T: UniformDataType
    {
        unsafe{self.pointers.UseProgram(*self.program)}
        value.to_uniform
        (
            &self.pointers,
            get_uniform_location
            (
                &self.pointers,
                &self.program,
                name
            ).unwrap()
        )
    }
    
    pub fn draw
    (
        &self, 
        origin: [i32; 2],
        resolution: [u32; 2]
    ) -> ()
    {
        unsafe
        {
            self.pointers.Viewport
            (
                origin[0],
                origin[1],
                resolution[0] as _,
                resolution[1] as _
            );
            self.pointers.UseProgram(*self.program);
            self.pointers.BindVertexArray(*self.vao);
            self.pointers.DrawElements
            (
                TRIANGLES, 
                6,
                UNSIGNED_INT,
                [0, 1, 2, 0, 3, 2].as_ptr() as _
            )
        }
    }
}

// ----------------------------------------------------------------------------------------------------

pub struct Filler{canvas: Canvas}

impl Filler
{
    pub fn new(pointers: &FunctionPointers) -> Self
    {
        Self
        {
            canvas: Canvas::new
            (
                pointers,
                &"
                #version 330 core
                in vec2 st;
                uniform vec4 input_color;
                out vec4 color;
                void main()
                {
                    color = input_color;
                }
                "
            )
        }
    }

    pub fn fill
    (
        &mut self,
        color: [f32; 4],
        origin: [i32; 2],
        resolution: [u32; 2]
    ) -> ()
    {
        self.canvas.set_uniform("input_color", color);
        self.canvas.draw(origin, resolution)
    }
}

// ----------------------------------------------------------------------------------------------------

pub struct Blitter
{
    pointers: FunctionPointers,
    canvas: Canvas,
    texture: Texture
}

impl Blitter
{
    pub fn new(pointers: &FunctionPointers) -> Self
    {    
        let canvas = Canvas::new
        (
            pointers,
            &"
            #version 330 core
            in vec2 st;
            out vec4 color;
            uniform sampler2D image;
            uniform ivec4 order;
            uniform float gamma;
            void main()
            {
                for(int channel = 0; channel < 4; channel++)
                {
                    color[channel] = pow
                    (
                        texture
                        (
                            image,
                            vec2(st.x, 1.0 - st.y)
                        )[order[channel]],
                        gamma
                    );
                }
            }
            "
        );
        canvas.set_uniform("image", 0i32);
        let texture = create_texture
        (
            pointers,
            None,
            InterpolationType::Linear,
            InterpolationType::Linear,
            Some(InterpolationType::Nearest)
        );
        Self
        {
            pointers: pointers.clone(),
            canvas,
            texture
        }
    }

    pub fn upload_texture<T: TextureBaseDataType>
    (
        &mut self, 
        image: Image<T>,
        channel_order: [i32; 4],
        gamma: f32
    ) -> ()
    {
        self.canvas.set_uniform("order", channel_order);
        self.canvas.set_uniform("gamma", gamma);
        fill_texture
        (
            &self.pointers,
            &self.texture,
            true,
            image
        );
    }

    pub fn blit(&self, resolution: [u32; 2]) -> ()
    {
        unsafe
        {
            self.pointers.ActiveTexture(TEXTURE0);
            self.pointers.BindTexture(TEXTURE_2D, *self.texture);
        }
        self.canvas.draw([0, 0], resolution)
    }
}

// ----------------------------------------------------------------------------------------------------

struct Glyph
{
    origin: (i32, i32),
    resolution: (usize, usize),
    pixels: Vec<u8>
}

// ----------------------------------------------------------------------------------------------------

struct FontRasterizer
{
    font: fontdue::Font,
    size: u16,
    units_per_em: f32,
    leading: f32
}

impl FontRasterizer
{
    fn new(font: &[u8], size: u16) -> Self
    {
        let font = fontdue::Font::from_bytes
        (
            font,
            fontdue::FontSettings
            {
                collection_index: 0,
                scale: size as _
            }
        ).unwrap();
        let units_per_em = font.units_per_em();
        let leading = font
            .horizontal_line_metrics(size as _)
            .unwrap()
            .new_line_size;
        Self{font, size, units_per_em, leading}
    }
    
    fn rasterize_glyph(&self, glyph_index: u16) -> Glyph
    {
        let (metrics, bitmap) = self.font
            .rasterize_indexed(glyph_index, self.size as _);
        Glyph
        {
            origin: (metrics.xmin, metrics.ymin),
            resolution: (metrics.width, metrics.height),
            pixels: bitmap
        }
    }
}

// ----------------------------------------------------------------------------------------------------

struct LineShaper(rustybuzz::Face<'static>);

impl LineShaper
{
    fn new(font: &'static [u8], size: u16) -> Self
    {
        let mut font = rustybuzz::Face::from_slice(font, 0).unwrap();
        font.set_points_per_em(Some(72.0));
        font.set_pixels_per_em(Some((size, size)));
        Self(font)
    }
    
    fn shape_line(&self, line: &str) -> Vec<(i32, u32)>
    {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.set_cluster_level(rustybuzz::BufferClusterLevel::MonotoneCharacters);
        buffer.set_direction(rustybuzz::Direction::LeftToRight);
        buffer.set_script(rustybuzz::script::LATIN);
        buffer.set_language(rustybuzz::Language::from_str("English").unwrap());
        buffer.push_str(line);
        let buffer = rustybuzz::shape(&self.0, &[], buffer);
        buffer.glyph_positions().iter()
            .zip(buffer.glyph_infos().iter())
            .map
            (
                |(position, info)|
                (
                    position.x_advance,
                    info.glyph_id
                )
            ).collect()
    }
}

// ----------------------------------------------------------------------------------------------------

struct Paragraph
{
    rasterizer: FontRasterizer,
    shaper: LineShaper,
    glyphs: Vec<Glyph>,
    dimensions: [u32; 2]
}

impl Paragraph
{
    fn new(font: &'static [u8], size: u16) -> Self
    {
        Self
        {
            rasterizer: FontRasterizer::new(font, size),
            shaper: LineShaper::new(font, size),
            glyphs: vec![],
            dimensions: Default::default()
        }
    }

    fn layout_glyphs(&mut self, text: &str, wrap: i32) -> ()
    {
        let paragraph = textwrap::fill(text, wrap as usize);
        let lines = paragraph.lines();
        let num_lines = lines.clone().count() - 1;
        let top = self.rasterizer.leading as i32 * num_lines as i32;
        let mut glyphs = vec![];
        for (line_index, line) in lines.enumerate()
        {
            let mut total_advance = 0;
            for (advance, id) in self.shaper.shape_line(&line)
            {
                let glyph = self.rasterizer.rasterize_glyph(id as _);
                let x = glyph.origin.0 + total_advance;
                let y = glyph.origin.1 + top -
                    self.rasterizer.leading as i32
                    * line_index as i32;
                glyphs.push(Glyph{origin: (x, y), ..glyph});
                total_advance += advance * self.rasterizer.size as i32 /
                    self.rasterizer.units_per_em as i32; // **
            }
        }
        self.glyphs = glyphs;
        let mut max = [0, 0];
        for Glyph{origin, resolution, ..} in &self.glyphs
        {
            max[0] = max[0].max((origin.0 + resolution.0 as i32) as u32);
            max[1] = max[1].max((origin.1 + resolution.1 as i32) as u32);
        }        
        self.dimensions = max
    }

    fn dimensions(&self) -> [u32; 2]
    {
        self.dimensions
    }
}

// ----------------------------------------------------------------------------------------------------

pub struct Typewriter
{
    pointers: FunctionPointers,
    paragraph: Paragraph,
    canvas: Canvas,
    texture: Texture,
    font: &'static [u8],
    text: String,
    wrap: i32
}

impl Typewriter
{
    pub fn new
    (
        pointers: &FunctionPointers,
        font: &'static [u8],
        size: u16
    ) -> Self
    {
        let canvas = Canvas::new
        (
            pointers,
            &"
            #version 330 core
            in vec2 st;
            uniform sampler2D glyph;
            uniform vec4 text_color;
            out vec4 color;
            void main()
            {
                color = vec4
                (
                    text_color.rgb,
                    texture
                    (
                        glyph,
                        vec2(st.x, 1.0 - st.y)
                    ).r * text_color.a
                );
            }
            "
        );
        canvas.set_uniform("glyph", 0i32);
        canvas.set_uniform("text_color", [0.0, 0.0, 0.0, 1.0]);
        Self
        {
            pointers: pointers.clone(),
            paragraph: Paragraph::new(font, size),
            canvas,
            texture: create_texture
            (
                pointers,
                None,
                InterpolationType::Linear,
                InterpolationType::Linear,
                None
            ),
            font,
            text: String::default(),
            wrap: 60
        }
    }

    pub fn layout_text(&mut self, text: &str, wrap: i32) -> ()
    {
        self.paragraph.layout_glyphs(text, wrap);
        self.text = text.to_string();
        self.wrap = wrap
    }
    
    pub fn dimensions(&self) -> [u32; 2]
    {
        self.paragraph.dimensions()
    }

    pub fn change_font_size(&mut self, size: u16) -> ()
    {
        let text = self.text.to_string();
        self.paragraph = Paragraph::new(self.font, size);
        self.layout_text(&text, self.wrap)
    } 

    pub fn draw(&mut self, origin: [i32; 2]) -> ()
    {        
        unsafe
        {
            self.pointers.ActiveTexture(TEXTURE0);
            self.pointers.BindTexture(TEXTURE_2D, *self.texture);
        }    
        for Glyph{origin: glyph_origin, resolution, pixels}
            in &self.paragraph.glyphs
        {
            let resolution = 
            [
                resolution.0 as u32,
                resolution.1 as u32
            ];
            fill_texture
            (
                &self.pointers,
                &self.texture,
                true, // **
                Image
                {
                    data: Some(&pixels),
                    resolution,
                    channel_count: ChannelCount::One
                }
            );
            self.canvas.draw
            (
                [
                    origin[0] + glyph_origin.0, 
                    origin[1] + glyph_origin.1
                ],
                resolution
            )
        }
    }
}
