use smash::{
    app::{self, lua_bind::*, sv_animcmd::{frame, wait}, *},
    hash40,
    lib::{lua_const::*, L2CValue, LuaConst},
    lua2cpp::*,
    phx::*,
};
use smash_script::{macros::*, *};
use smashline::*;

mod aerials;
mod ground;
mod smashes;
mod specials;
mod tilts;
pub mod weapons;

pub fn install(agent: &mut Agent) {
    ground::install(agent);
    tilts::install(agent);
    smashes::install(agent);
    aerials::install(agent);
    specials::install(agent);
}
