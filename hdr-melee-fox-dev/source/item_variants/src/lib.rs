#![allow(non_snake_case)]

//! Bumper and soccer ball variants.
//!
//! The game has no way to add new item kinds, so instead every bumper and soccer ball that
//! spawns rolls a random variant (kept for that item's whole life):
//!
//! | Item        | Variant | Look                   | Effect                                   |
//! |-------------|---------|------------------------|------------------------------------------|
//! | Bumper      | normal  | NormalBumper texture   | unchanged, custom bounce sound           |
//! | Bumper      | weak    | SpongeBumper texture   | knockback x0.5, custom bounce sound      |
//! | Bumper      | strong  | TuffBumper texture     | knockback x1.5, custom bounce sound      |
//! | Soccer ball | normal  | NormalSoccer texture   | unchanged                                |
//! | Soccer ball | spike   | SpikeSoccer (purple)   | launches down (280 degrees)              |
//! | Soccer ball | launch  | LaunchSoccer (yellow)  | launches straight up (90 degrees)        |
//!
//! Everything runs from one per-frame callback on every fighter (smashline "fighter" agent).
//!
//! The bumper looks and sounds come from the `hdr-item-variants` ARCropolis mod:
//! - The bumper model has three copies of its mesh: `normal_model`, `weak_model` and
//!   `tuff_model`, each with its own texture. Each bumper shows one of them. The soccer ball
//!   works the same way with four copies (`normal_`/`spike_`/`launch_`/`gravity_gamemodel`);
//!   the gravity mesh is always hidden (that variant was removed). Its textures are edited in
//!   Project TUFF\soccer-texture.
//! - The stock bounce sound `se_item_bumper_repel` is replaced with silence. When it starts on a
//!   bumper, the plugin plays that bumper's own sound instead. The three sounds live in unused
//!   Death's Scythe slots of the item sound bank (keep that item off).

use skyline::nn::ro::LookupSymbol;
use smash::app::{self, lua_bind::*, BattleObject, BattleObjectModuleAccessor};
use smash::lib::lua_const::*;
use smash::lua2cpp::L2CFighterCommon;
use smash::phx::Hash40;
use smashline::*;

// ---- tuning ---------------------------------------------------------------------------------

/// Bumper variants: normal, weak, strong (equal odds).
const BUMPER_VARIANTS: u32 = 3;
const WEAK_BUMPER_KB_MUL: f32 = 0.5;
const STRONG_BUMPER_KB_MUL: f32 = 1.5;

/// Mesh and bounce sound for each bumper variant (normal, weak, strong).
const BUMPER_MESHES: [&str; 3] = ["normal_model", "weak_model", "tuff_model"];
const BUMPER_SOUNDS: [&str; 3] = ["se_item_deathscythe_swing_l", "se_item_deathscythe_swing_m", "se_item_deathscythe_reap"];

/// Soccer ball variants: normal, spike, launch (equal odds).
const SOCCER_VARIANTS: u32 = 3;
const SPIKE_ANGLE: i32 = 280;
/// Mesh for each soccer ball variant (normal, spike, launch). The model still has a gravity
/// mesh from the removed gravity variant; listing it here keeps it hidden.
const SOCCER_MESHES: [&str; 4] = ["normal_gamemodel", "spike_gamemodel", "launch_gamemodel", "gravity_gamemodel"];
const LAUNCH_ANGLE: i32 = 90;


// ---- item manager ---------------------------------------------------------------------------

static mut ITEM_MANAGER: *const *mut app::ItemManager = std::ptr::null();

unsafe fn item_manager() -> *mut app::ItemManager {
    if ITEM_MANAGER.is_null() {
        let mut sym = 0usize;
        LookupSymbol(
            &mut sym as *mut usize,
            "_ZN3lib9SingletonIN3app11ItemManagerEE9instance_E\0".as_ptr() as _,
        );
        if sym == 0 {
            return std::ptr::null_mut();
        }
        ITEM_MANAGER = sym as _;
    }
    *ITEM_MANAGER
}

// ---- per-item state -------------------------------------------------------------------------

/// Object ids restart every match, so an entry is dropped as soon as its item is no longer
/// active (checked once per scan of the item list). This doesn't depend on time, so pausing
/// (e.g. camera mode) can't make an item re-roll.
const MAX_TRACKED: usize = 32;

#[derive(Clone, Copy)]
struct ItemState {
    used: bool,
    id: u32,
    variant: u32,
    bounce_playing: bool,
    /// Scan number when this item was last seen.
    seen_scan: u64,
}

static mut ITEMS: [ItemState; MAX_TRACKED] = [ItemState { used: false, id: 0, variant: 0, bounce_playing: false, seen_scan: 0 }; MAX_TRACKED];
static mut SCAN: u64 = 0;
static mut RNG: u64 = 0;

fn now() -> u64 {
    unsafe { skyline::nn::os::GetSystemTick() }
}

/// xorshift64*, seeded from the system clock on first use.
unsafe fn random(n: u32) -> u32 {
    if RNG == 0 {
        RNG = now() | 1;
    }
    RNG ^= RNG >> 12;
    RNG ^= RNG << 25;
    RNG ^= RNG >> 27;
    ((RNG.wrapping_mul(0x2545F4914F6CDD1D) >> 33) % n as u64) as u32
}

/// This item's state, rolling a random variant in 0..n the first time it's seen.
unsafe fn item_state(id: u32, n: u32) -> &'static mut ItemState {
    let items = &mut *std::ptr::addr_of_mut!(ITEMS);
    if let Some(i) = items.iter().position(|e| e.used && e.id == id) {
        items[i].seen_scan = SCAN;
        return &mut items[i];
    }
    // free slot, or (if somehow full) the one seen longest ago
    let slot = match items.iter().position(|e| !e.used) {
        Some(i) => &mut items[i],
        None => items.iter_mut().min_by_key(|e| e.seen_scan).unwrap(),
    };
    *slot = ItemState { used: true, id, variant: random(n), bounce_playing: false, seen_scan: SCAN };
    log(&format!("item {:#x} rolled variant {}", id, slot.variant));
    slot
}

/// Drops entries for items that weren't in this scan (despawned).
unsafe fn forget_missing_items() {
    for e in (*std::ptr::addr_of_mut!(ITEMS)).iter_mut() {
        if e.used && e.seen_scan != SCAN {
            e.used = false;
        }
    }
}

// ---- helpers --------------------------------------------------------------------------------

/// Shows only this variant's mesh (one of `meshes`).
unsafe fn show_mesh(boma: *mut BattleObjectModuleAccessor, meshes: &[&str], variant: u32) {
    for (i, mesh) in meshes.iter().enumerate() {
        ModelModule::set_mesh_visibility(boma, Hash40::new(mesh), i as u32 == variant);
    }
}

/// The stock bounce sound is silent; when it starts, play this bumper's own sound once.
unsafe fn bounce_sound(boma: *mut BattleObjectModuleAccessor, state: &mut ItemState) {
    let playing = SoundModule::is_playing(boma, Hash40::new("se_item_bumper_repel"));
    if playing && !state.bounce_playing {
        let se = BUMPER_SOUNDS[state.variant as usize];
        SoundModule::play_se(boma, Hash40::new(se), true, false, false, false, app::enSEType(0));
        log(&format!("item {:#x} bounce -> {}", state.id, se));
    }
    state.bounce_playing = playing;
}

/// Changes the launch angle of the item's active hitbox (soccer balls only use id 0).
unsafe fn set_angle(boma: *mut BattleObjectModuleAccessor, angle: i32) {
    if AttackModule::is_attack(boma, 0, false) {
        AttackModule::set_vector(boma, 0, angle, false);
    }
}

// ---- debug log ------------------------------------------------------------------------------

/// Appends lines to sd:/item_variants_log.txt (capped). Set to false once everything is tested.
const DEBUG_LOG: bool = true;
const DEBUG_LOG_MAX_LINES: u32 = 300;
static mut LOG_LINES: u32 = 0;

fn log(msg: &str) {
    unsafe {
        if !DEBUG_LOG || LOG_LINES >= DEBUG_LOG_MAX_LINES {
            return;
        }
        LOG_LINES += 1;
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("sd:/item_variants_log.txt") {
        let _ = writeln!(f, "{}", msg);
    }
}

// ---- per-frame ------------------------------------------------------------------------------

unsafe extern "C" fn item_variants_frame(_fighter: &mut L2CFighterCommon) {
    let manager = item_manager();
    if manager.is_null() {
        return;
    }

    SCAN += 1;
    let count = smash::app::lua_bind::ItemManager::get_num_of_active_item_all(manager);
    for i in 0..count {
        let item = smash::app::lua_bind::ItemManager::get_active_item(manager, i) as *mut BattleObject;
        if item.is_null() {
            continue;
        }
        let boma = (*item).module_accessor;
        if boma.is_null() {
            continue;
        }
        let id = (*item).battle_object_id;
        let kind = app::utility::get_kind(&mut *boma);

        if kind == *ITEM_KIND_BUMPER {
            let state = item_state(id, BUMPER_VARIANTS);
            show_mesh(boma, &BUMPER_MESHES, state.variant);
            match state.variant {
                1 => AttackModule::set_reaction_mul(boma, WEAK_BUMPER_KB_MUL),
                2 => AttackModule::set_reaction_mul(boma, STRONG_BUMPER_KB_MUL),
                _ => {}
            }
            bounce_sound(boma, state);
        } else if kind == *ITEM_KIND_SOCCERBALL {
            let variant = item_state(id, SOCCER_VARIANTS).variant;
            show_mesh(boma, &SOCCER_MESHES, variant);
            match variant {
                1 => set_angle(boma, SPIKE_ANGLE),
                2 => set_angle(boma, LAUNCH_ANGLE),
                _ => {}
            }
        }
    }
    forget_missing_items();
}

#[skyline::main(name = "item_variants")]
pub fn main() {
    log("item_variants loaded");
    Agent::new("fighter")
        .on_line(Main, item_variants_frame)
        .install();
}
