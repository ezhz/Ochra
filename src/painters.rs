
use super::ogl::*;

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
