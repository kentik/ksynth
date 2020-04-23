use capnp::struct_list::Builder;
use crate::chf_capnp::custom::{Owned, value};

pub struct Customs<'a> {
    builder: Builder<'a, Owned>,
    index:   u32,
}

impl<'a> Customs<'a> {
    pub fn new(b: Builder<'a, Owned>) -> Self {
        Self {
            builder: b,
            index:   0,
        }
    }

    pub fn next<F: Fn(&mut value::Builder)>(&mut self, id: u32, f: F) {
        let mut custom = self.builder.reborrow().get(self.index);
        custom.set_id(id);
        f(&mut custom.init_value());
        self.index += 1;
    }
}
