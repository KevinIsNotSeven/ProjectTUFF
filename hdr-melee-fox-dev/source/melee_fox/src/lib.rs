#![allow(unused)]
#![allow(non_snake_case)]

//! Melee Fox: overrides selected HDR Fox scripts.
//! HDR registers its Fox scripts at `Priority::Low`; ours use `Priority::High`,
//! so smashline keeps ours no matter which plugin loads first.

mod acmd;

#[skyline::main(name = "melee_fox")]
pub fn main() {
    let fox = &mut smashline::Agent::new("fox");
    acmd::install(fox);
    fox.install();

    let blaster_bullet = &mut smashline::Agent::new("fox_blaster_bullet");
    acmd::weapons::install_blaster_bullet(blaster_bullet);
    blaster_bullet.install();

    let illusion = &mut smashline::Agent::new("fox_illusion");
    acmd::weapons::install_illusion(illusion);
    illusion.install();
}
