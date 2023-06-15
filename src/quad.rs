
use super::vector::*;

// ----------------------------------------------------------------------------------------------------

pub type Vector2 = Vector<f64, 2>;

// ----------------------------------------------------------------------------------------------------

pub trait BBox
{
    fn min(&self) -> Vector2;
    
    fn max(&self) -> Vector2;
    
    fn mid(&self) -> Vector2
    {
        self.min() + self.size() / 2.0
    }
    
    fn size(&self) -> Vector2
    {
        self.max() - self.min()
    }
}

// ----------------------------------------------------------------------------------------------------

pub trait XForm
{
    fn translate(&mut self, offset: Vector2) -> ();
    fn scale(&mut self, scale: Vector2) -> ();
}

// ----------------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum AlignPosition{Start, Middle, End}

pub use AlignPosition::*;

// ----------------------------------------------------------------------------------------------------

pub trait Align: BBox + XForm
{
    fn align
    (
        &mut self, other: &(impl BBox + XForm),
        horizontal: Option<[AlignPosition; 2]>,
        vertical: Option<[AlignPosition; 2]>
    ) -> ()
    {
        let mut offset = Vector([0.0, 0.0]);
        for (index, direction) in
            [horizontal, vertical].iter().enumerate()
        {     
            if let Some([source, target]) = &direction
            {
                offset[index] = match target
                {
                    Start => other.min()[index],
                    Middle => other.mid()[index],
                    End => other.max()[index]
                } - match source
                {
                    Start => 0.0,
                    Middle => self.size()[index] * 0.5,
                    End => self.size()[index]
                } - self.min()[index];
            }  
        }
        self.translate(offset)
    }
    
    fn center
    (
        &mut self, 
        other: &(impl BBox + XForm)
    ) -> ()
    {
        self.align
        (
            other,
            Some([Middle, Middle]),
            Some([Middle, Middle])
        )
    }
}

impl<T: BBox + XForm> Align for T {}

// ----------------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Quad([Vector2; 2]);

impl Quad
{
    pub fn new(min: [f64; 2], size: [f64; 2]) -> Self
    {
        assert![!(..0.0f64).contains(&size[0])];
        assert![!(..0.0f64).contains(&size[1])];
        Self
        ([
            Vector(min),
            Vector(min) + Vector(size)
        ])
    }
}

impl BBox for Quad
{
    fn min(&self) -> Vector2 {self.0[0]}
    fn max(&self) -> Vector2 {self.0[1]}
}

impl XForm for Quad
{
    fn translate(&mut self, offset: Vector2) -> ()
    {
        for vector in &mut self.0
        {
            *vector += offset
        }
    }
    
    fn scale(&mut self, scale: Vector2) -> ()
    {
        for vector in &mut self.0
        {
            *vector *= scale
        }
    }
}
