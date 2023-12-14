use super::component::Component;

pub struct ReadResult<'a, T: Component> {
    pub components: Vec<&'a T>,
}

pub struct WriteResult<'a, T: Component> {
    pub components: Vec<&'a mut T>,
}

impl<'a, T: Component> Iterator for ReadResult<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.components.pop()
    }
}

impl<'a, T: Component> Iterator for WriteResult<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        self.components.pop()
    }
}
