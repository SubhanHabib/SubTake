//! Plain data carried between the controller and the views: shared lists,
//! colours and RGBA images.

use super::*;

pub type SharedString = String;
#[derive(Clone, Debug, PartialEq)]
pub struct ModelRc<T>(Rc<Vec<T>>);
impl<T> Default for ModelRc<T> {
    fn default() -> Self {
        Self(Rc::new(Vec::new()))
    }
}

impl<T: Clone> ModelRc<T> {
    pub fn new(model: VecModel<T>) -> Self {
        Self(Rc::new(model.0))
    }

    pub fn row_count(&self) -> usize {
        self.0.len()
    }

    pub fn row_data(&self, row: usize) -> Option<T> {
        self.0.get(row).cloned()
    }

    pub fn iter(&self) -> std::vec::IntoIter<T> {
        self.0.as_ref().clone().into_iter()
    }
}

pub struct VecModel<T>(Vec<T>);
impl<T> From<Vec<T>> for VecModel<T> {
    fn from(items: Vec<T>) -> Self {
        Self(items)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color(u32);
impl Color {
    pub fn from_rgb_u8(red: u8, green: u8, blue: u8) -> Self {
        Self(((red as u32) << 16) | ((green as u32) << 8) | blue as u32)
    }

    pub fn to_gpui(self) -> gpui::Hsla {
        gpui::rgb(self.0).into()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Image(pub Option<Arc<gpui::RenderImage>>);
impl Image {
    pub fn from_rgba8(buffer: SharedPixelBuffer<Rgba8Pixel>) -> Self {
        let mut bytes = buffer.bytes;
        // GPUI's RenderImage stores BGRA, while the shared compositor emits RGBA.
        for pixel in bytes.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let image = image::RgbaImage::from_raw(buffer.width, buffer.height, bytes)
            .expect("validated pixel buffer");
        Self(Some(Arc::new(gpui::RenderImage::new(smallvec::smallvec![
            image::Frame::new(image)
        ]))))
    }

    /// Rows already in GPUI's BGRA, as a camera hands them over; nothing
    /// for bytes that are not `width` × `height` pixels.
    pub fn from_bgra8(bytes: Vec<u8>, width: u32, height: u32) -> Self {
        Self(
            image::RgbaImage::from_raw(width, height, bytes).map(|image| {
                Arc::new(gpui::RenderImage::new(smallvec::smallvec![
                    image::Frame::new(image)
                ]))
            }),
        )
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        let image = image::open(path)?.into_rgba8();
        Ok(Self::from_rgba8(SharedPixelBuffer::clone_from_slice(
            image.as_raw(),
            image.width(),
            image.height(),
        )))
    }
}

#[derive(Clone, Copy, Default)]
pub struct Rgba8Pixel;
#[derive(Clone)]
pub struct SharedPixelBuffer<T> {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    marker: PhantomData<T>,
}

impl<T> SharedPixelBuffer<T> {
    pub fn clone_from_slice(bytes: &[u8], width: u32, height: u32) -> Self {
        assert_eq!(bytes.len(), width as usize * height as usize * 4);
        Self {
            bytes: bytes.to_vec(),
            width,
            height,
            marker: PhantomData,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}
