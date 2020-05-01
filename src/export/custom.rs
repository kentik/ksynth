use capnp::struct_list::Builder;
use log::error;
use crate::chf_capnp::c_h_f;
use crate::chf_capnp::custom::{Owned, value};

pub struct Customs<'a> {
    list:  Builder<'a, Owned>,
    name:  &'static str,
    count: u32,
    index: u32,
}

impl<'a> Customs<'a> {
    pub fn new(name: &'static str, msg: c_h_f::Builder<'a>, n: u32) -> Self {
        Self {
            list:  msg.init_custom(n),
            name:  name,
            count: n,
            index: 0,
        }
    }

    pub fn next<F: Fn(&mut value::Builder)>(&mut self, id: u32, f: F) {
        if self.index < self.count {
            let mut item = self.list.reborrow().get(self.index);
            item.set_id(id);
            f(&mut item.init_value());
            self.index += 1;
        } else {
            error!("{}: column {} out of bounds", self.name, self.index);
        }
    }
}
