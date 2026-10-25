//! Ivysaur, Project M style, layered on HDR's Ivysaur:
//! - **Neutral B = Synthesis / Solar Beam** (PM). Synthesis: a little wind on startup, then
//!   holding B heals and builds bulb charge at the same rate (SYNTH_PER_FRAME), and keeps healing
//!   at full charge until you let go. Landing cancels the ending in the air. With a full charge
//!   (CHARGE_MAX points) the next neutral B is a one-time Solar Beam: a long electric beam
//!   diagonally up-forward from the bulb, 25%, intangible while firing, helpless if used in the air.
//! - **Bulb hits heal and charge** (PM): up smash, up air, down air, up throw, pummel.
//! - **Up smash** comes out on frame 13 like PM's (HDR's hitbox otherwise).
//! - **Down B = Seed Bomb**: HDR's vertical seed (what HDR had on neutral B). One at a time.
//! - **Down taunt = Pokemon switch** (PM has no switch; this keeps it available).
//! - **Tether (up B):** faster startup, ledge-grab window frames 11-32 instead of 12-14, and a
//!   lift the first time it's used in the air (once per airtime).
//!
//! How: smashline status scripts have no priority and HDR's plugin registers last, so the
//! status-table slots are written directly every frame (same trick as K. Rool), keeping HDR's
//! functions to call: SPECIAL_LW pre redirects to SPECIAL_N (seed) unless the switch was asked
//! for from the taunt; SPECIAL_N main is Synthesis / Solar Beam unless it came from down B.
use super::*;
use smash::app::sv_animcmd::{frame, wait};
use smash::hash40;
use smash::lua2cpp::{L2CAgentBase, L2CFighterBase};
use smash::phx::{Hash40, Vector3f};
use smash_script::macros::*;

type CVoid = skyline::libc::c_void;
type StatusFn = unsafe extern "C" fn(&mut L2CFighterCommon) -> L2CValue;

const MAX_ENTRIES: usize = 8;
/// Bulb charge needed for Solar Beam, in % healed (PM: 18-22).
const CHARGE_MAX: f32 = 18.0;
/// Synthesis heal and charge per frame while holding B (0.025 = 1.5% per second; PM ~1%).
const SYNTH_PER_FRAME: f32 = 0.025;
/// Synthesis starts healing this many frames into the move.
const SYNTH_HEAL_FROM: i32 = 14;
/// Synthesis ending lag after letting go (cancelled by landing).
const SYNTH_END_FRAMES: i32 = 10;
/// Solar Beam timeline (frames into the move) - PM: hitbox frames 20-41, helpless after.
const BEAM_INTANGIBLE_FROM: i32 = 4;
const BEAM_HIT_FROM: i32 = 20;
const BEAM_HIT_TO: i32 = 41;
const BEAM_END: i32 = 58;
const BEAM_DAMAGE: f32 = 25.0;
/// Beam length from the bulb (it's long: most stages' upper blast line from the ground).
const BEAM_LENGTH: f32 = 160.0;
/// Bulb-hit heal + charge (PM: up smash 9, up air 7, down air 7, up throw 6, pummel 2 - PM only
/// counts the sweetspots; these count any hit, so they're a bit lower).
const HEAL_UP_SMASH: f32 = 6.0;
const HEAL_UP_AIR: f32 = 4.0;
const HEAL_DOWN_AIR: f32 = 4.0;
const HEAL_UP_THROW: f32 = 5.0;
const HEAL_PUMMEL: f32 = 1.5;
/// Up B: motion rate during the startup (HDR plays frames 1-12 in 16 frames = 0.75).
const TETHER_STARTUP_RATE: f32 = 1.5;
/// Up B: ledge-grab window (animation frames; HDR: 12-14).
const TETHER_CHECK_FROM: f32 = 11.0;
const TETHER_CHECK_TO: f32 = 32.0;
/// Up B: upward speed added the first time per airtime.
const TETHER_LIFT: f32 = 1.4;

#[derive(Copy, Clone, PartialEq)]
enum Phase {
    SynthStart,
    SynthHold,
    SynthEnd,
    Beam,
}

static mut SEED_MODE: [bool; MAX_ENTRIES] = [false; MAX_ENTRIES];
static mut SWITCH_REQ: [bool; MAX_ENTRIES] = [false; MAX_ENTRIES];
static mut CHARGE: [f32; MAX_ENTRIES] = [0.0; MAX_ENTRIES];
static mut PHASE: [Phase; MAX_ENTRIES] = [Phase::SynthStart; MAX_ENTRIES];
static mut TIMER: [i32; MAX_ENTRIES] = [0; MAX_ENTRIES];
static mut PHASE_TIMER: [i32; MAX_ENTRIES] = [0; MAX_ENTRIES];
static mut LIFT_USED: [bool; MAX_ENTRIES] = [false; MAX_ENTRIES];
static mut PREV_HIT: [bool; MAX_ENTRIES] = [false; MAX_ENTRIES];
static mut PREV_STATUS: [i32; MAX_ENTRIES] = [-1; MAX_ENTRIES];
static mut GLOW_TIMER: [i32; MAX_ENTRIES] = [0; MAX_ENTRIES];
// HDR's (or the game's) functions that were in the slots we took over.
static mut SAVED_N_MAIN: [usize; MAX_ENTRIES] = [0; MAX_ENTRIES];
static mut SAVED_N_END: [usize; MAX_ENTRIES] = [0; MAX_ENTRIES];
static mut SAVED_LW_PRE: [usize; MAX_ENTRIES] = [0; MAX_ENTRIES];

unsafe fn entry(boma: *mut BattleObjectModuleAccessor) -> usize {
    (WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize).min(MAX_ENTRIES - 1)
}

unsafe fn call_saved(ptr: usize, fighter: &mut L2CFighterCommon) -> L2CValue {
    if ptr == 0 {
        return L2CValue::I32(0);
    }
    let f: StatusFn = std::mem::transmute(ptr);
    f(fighter)
}

/// Heal Ivysaur and add the same amount of bulb charge.
unsafe fn heal_and_charge(boma: *mut BattleObjectModuleAccessor, amount: f32) {
    let e = entry(boma);
    let dmg = DamageModule::damage(boma, 0);
    if dmg > 0.0 {
        DamageModule::add_damage(boma, -amount.min(dmg), 0);
    }
    let was_full = CHARGE[e] >= CHARGE_MAX;
    CHARGE[e] = (CHARGE[e] + amount).min(CHARGE_MAX);
    if !was_full && CHARGE[e] >= CHARGE_MAX {
        effect_follow(boma, "sys_level_up", "top", 0.0, 0.0, 0.0, 1.0);
        SoundModule::play_se(boma, Hash40::new("se_common_spirits_critical_l_tail"), true, false, false, false, app::enSEType(0));
    }
}

unsafe fn effect_follow(boma: *mut BattleObjectModuleAccessor, name: &str, bone: &str, y: f32, z: f32, rot_x: f32, scale: f32) -> u64 {
    let pos = Vector3f { x: 0.0, y, z };
    let rot = Vector3f { x: rot_x, y: 0.0, z: 0.0 };
    EffectModule::req_follow(boma, Hash40::new(name), Hash40::new(bone), &pos, &rot, scale, true, 0, 0, 0, 0, 0, false, false)
}

// ---- status table ---------------------------------------------------------------------------------

unsafe fn take_slot(base: &mut L2CFighterBase, kind: i32, line: StatusLine, ours: StatusFn, saved: &mut usize) {
    let have = base.sv_get_status_func(&L2CValue::I32(kind), &L2CValue::I32(line as i32)).get_ptr() as usize;
    let ours = ours as usize;
    if have != ours {
        *saved = have;
        base.sv_set_status_func(L2CValue::I32(kind), L2CValue::I32(line as i32), &mut *(ours as *mut CVoid));
    }
}

unsafe extern "C" fn use_special_n(_fighter: &mut L2CFighterCommon) -> L2CValue {
    // HDR blocks neutral B while a seed is out; Synthesis doesn't need that.
    L2CValue::Bool(true)
}

unsafe extern "C" fn use_special_lw(fighter: &mut L2CFighterCommon) -> L2CValue {
    // down B is the seed now: one at a time
    L2CValue::Bool(!ArticleModule::is_exist(fighter.module_accessor, *FIGHTER_PFUSHIGISOU_GENERATE_ARTICLE_SEED))
}

unsafe fn take_over(fighter: &mut L2CFighterCommon) {
    let e = entry(fighter.module_accessor);
    let base = &mut *(fighter as *mut L2CFighterCommon as *mut L2CFighterBase);
    take_slot(base, *FIGHTER_STATUS_KIND_SPECIAL_LW, StatusLine::Pre, special_lw_pre, &mut SAVED_LW_PRE[e]);
    take_slot(base, *FIGHTER_STATUS_KIND_SPECIAL_N, StatusLine::Main, special_n_main, &mut SAVED_N_MAIN[e]);
    take_slot(base, *FIGHTER_STATUS_KIND_SPECIAL_N, StatusLine::End, special_n_end, &mut SAVED_N_END[e]);
    fighter.global_table[0x38].assign(&L2CValue::Ptr(use_special_n as *const () as _)); // USE_SPECIAL_N_CALLBACK
    fighter.global_table[0x3B].assign(&L2CValue::Ptr(use_special_lw as *const () as _)); // USE_SPECIAL_LW_CALLBACK
}

unsafe extern "C" fn special_lw_pre(fighter: &mut L2CFighterCommon) -> L2CValue {
    let e = entry(fighter.module_accessor);
    if SWITCH_REQ[e] {
        return call_saved(SAVED_LW_PRE[e], fighter); // Pokemon switch (from the down taunt)
    }
    SEED_MODE[e] = true;
    StatusModule::set_status_kind_interrupt(fighter.module_accessor, *FIGHTER_STATUS_KIND_SPECIAL_N);
    L2CValue::I32(1)
}

unsafe extern "C" fn special_n_main(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let e = entry(boma);
    if SEED_MODE[e] {
        return call_saved(SAVED_N_MAIN[e], fighter); // HDR's seed
    }
    TIMER[e] = 0;
    PHASE_TIMER[e] = 0;
    PHASE[e] = if CHARGE[e] >= CHARGE_MAX { Phase::Beam } else { Phase::SynthStart };
    set_kinetic(boma, PHASE[e] == Phase::Beam);
    set_motion(boma, "special_n_start", "special_air_n_start");
    fighter.sub_shift_status_main(L2CValue::Ptr(special_n_loop as *const () as _))
}

unsafe extern "C" fn special_n_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let e = entry(boma);
    if !SEED_MODE[e] {
        HitModule::set_whole(boma, app::HitStatus(*HIT_STATUS_NORMAL), 0);
        AttackModule::clear_all(boma);
    }
    SEED_MODE[e] = false;
    call_saved(SAVED_N_END[e], fighter)
}

// ---- Synthesis / Solar Beam -----------------------------------------------------------------------

unsafe fn set_kinetic(boma: *mut BattleObjectModuleAccessor, beam: bool) {
    if StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND {
        GroundModule::correct(boma, app::GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_GROUND_STOP);
    } else {
        GroundModule::correct(boma, app::GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
        // the beam holds Ivysaur in place in the air; Synthesis drifts down
        KineticModule::change_kinetic(boma, if beam { *FIGHTER_KINETIC_TYPE_AIR_STOP } else { *FIGHTER_KINETIC_TYPE_FALL });
    }
}

unsafe fn set_motion(boma: *mut BattleObjectModuleAccessor, ground: &str, air: &str) {
    let name = if StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND { ground } else { air };
    MotionModule::change_motion(boma, Hash40::new(name), 0.0, 1.0, false, 0.0, false, false);
}

unsafe fn to_idle(fighter: &mut L2CFighterCommon, helpless: bool) {
    let kind = if StatusModule::situation_kind(fighter.module_accessor) == *SITUATION_KIND_GROUND {
        *FIGHTER_STATUS_KIND_WAIT
    } else if helpless {
        *FIGHTER_STATUS_KIND_FALL_SPECIAL
    } else {
        *FIGHTER_STATUS_KIND_FALL
    };
    fighter.change_status(L2CValue::I32(kind), L2CValue::Bool(false));
}

unsafe fn synthesis_wind(fighter: &mut L2CFighterCommon) {
    // PM: "a relatively weak wind box" on startup
    ATTACK(fighter, 0, 0, Hash40::new("top"), 0.0, 361, 0, 0, 40, 9.0, 0.0, 7.0, 4.0, Some(0.0), Some(7.0), Some(-4.0), 0.0, 0.0, *ATTACK_SETOFF_KIND_OFF, *ATTACK_LR_CHECK_POS, false, 0, 0.0, 0, true, true, false, false, false, *COLLISION_SITUATION_MASK_GA, *COLLISION_CATEGORY_MASK_ALL, *COLLISION_PART_MASK_ALL, false, Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_S, *COLLISION_SOUND_ATTR_NONE, *ATTACK_REGION_NONE);
}

unsafe fn beam_hitboxes(fighter: &mut L2CFighterCommon) {
    // three capsules along the 45-degree line from the bulb (z is forward on "top")
    let d = std::f32::consts::FRAC_1_SQRT_2;
    let seg = BEAM_LENGTH / 3.0;
    for i in 0..3u64 {
        let a = 4.0 + seg * i as f32;
        let b = 4.0 + seg * (i + 1) as f32;
        let dmg = if i == 0 { BEAM_DAMAGE } else { BEAM_DAMAGE - 3.0 };
        ATTACK(fighter, i, 0, Hash40::new("top"), dmg, 45, 92, 0, 55, 6.5, 0.0, 13.0 + a * d, 4.0 + a * d, Some(0.0), Some(13.0 + b * d), Some(4.0 + b * d), 1.4, 1.0, *ATTACK_SETOFF_KIND_ON, *ATTACK_LR_CHECK_F, false, 0, 0.0, 0, false, false, false, false, true, *COLLISION_SITUATION_MASK_GA, *COLLISION_CATEGORY_MASK_ALL, *COLLISION_PART_MASK_ALL, false, Hash40::new("collision_attr_elec"), *ATTACK_SOUND_LEVEL_L, *COLLISION_SOUND_ATTR_ELEC, *ATTACK_REGION_ENERGY);
    }
}

unsafe fn beam_effects(boma: *mut BattleObjectModuleAccessor) {
    // streaks laid end to end along the beam, plus sparks; refreshed while it's active
    let d = std::f32::consts::FRAC_1_SQRT_2;
    let mut s = 8.0;
    while s < BEAM_LENGTH {
        let h = effect_follow(boma, "sys_attack_line", "top", 13.0 + s * d, 4.0 + s * d, -45.0, 2.2);
        EffectModule::set_rgb(boma, h as u32, 1.0, 1.0, 0.55);
        if (s as i32 / 16) % 2 == 0 {
            let h = effect_follow(boma, "sys_hit_elec_s", "top", 13.0 + s * d, 4.0 + s * d, 0.0, 0.8);
            EffectModule::set_rgb(boma, h as u32, 0.9, 1.0, 0.5);
        }
        s += 16.0;
    }
}

unsafe extern "C" fn special_n_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let e = entry(boma);
    let beam = PHASE[e] == Phase::Beam;
    if !StatusModule::is_changing(boma) && StatusModule::is_situation_changed(boma) {
        if !beam && StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND && PHASE[e] != Phase::SynthHold {
            to_idle(fighter, false); // landing cancels Synthesis' start/end
            return L2CValue::I32(0);
        }
        set_kinetic(boma, beam);
        // swap to the ground/air version of the same animation, same frame
        let m = MotionModule::motion_kind(boma);
        let start = m == hash40("special_n_start") || m == hash40("special_air_n_start");
        let ground = StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND;
        let name = match (start, ground) {
            (true, true) => "special_n_start",
            (true, false) => "special_air_n_start",
            (false, true) => "special_n",
            (false, false) => "special_air_n",
        };
        MotionModule::change_motion_inherit_frame(boma, Hash40::new(name), -1.0, 1.0, 0.0, false, false);
    }
    if StopModule::is_stop(boma) {
        return L2CValue::I32(0);
    }
    TIMER[e] += 1;
    PHASE_TIMER[e] += 1;
    let t = TIMER[e];
    // after the bulb opens, hold the bulb-up pose
    if MotionModule::is_end(boma) {
        set_motion(boma, "special_n", "special_air_n");
    }

    if beam {
        if t == BEAM_INTANGIBLE_FROM {
            HitModule::set_whole(boma, app::HitStatus(*HIT_STATUS_XLU), 0);
        }
        if t == BEAM_HIT_FROM - 6 {
            effect_follow(boma, "sys_smash_flash", "flower", 0.0, 0.0, 0.0, 2.0);
            SoundModule::play_se(boma, Hash40::new("se_common_electric_hit_l"), true, false, false, false, app::enSEType(0));
        }
        if t == BEAM_HIT_FROM {
            CHARGE[e] = 0.0;
            beam_hitboxes(fighter);
            CameraModule::req_quake(boma, *CAMERA_QUAKE_KIND_M);
            SoundModule::play_se(boma, Hash40::new("se_common_bomb_l"), true, false, false, false, app::enSEType(0));
        }
        if (BEAM_HIT_FROM..=BEAM_HIT_TO).contains(&t) && (t - BEAM_HIT_FROM) % 5 == 0 {
            beam_effects(boma);
        }
        if t == BEAM_HIT_TO + 1 {
            AttackModule::clear_all(boma);
            HitModule::set_whole(boma, app::HitStatus(*HIT_STATUS_NORMAL), 0);
        }
        if t >= BEAM_END {
            to_idle(fighter, true);
        }
        return L2CValue::I32(0);
    }

    // Synthesis
    if t == 3 {
        synthesis_wind(fighter);
    }
    if t == 6 {
        AttackModule::clear_all(boma);
    }
    let holding = ControlModule::check_button_on(boma, *CONTROL_PAD_BUTTON_SPECIAL);
    match PHASE[e] {
        Phase::SynthStart => {
            if t >= SYNTH_HEAL_FROM {
                PHASE[e] = if holding { Phase::SynthHold } else { Phase::SynthEnd };
                PHASE_TIMER[e] = 0;
            }
        }
        Phase::SynthHold => {
            if ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_GUARD) {
                to_idle(fighter, false);
                return L2CValue::I32(0);
            }
            if !holding {
                PHASE[e] = Phase::SynthEnd;
                PHASE_TIMER[e] = 0;
            } else {
                heal_and_charge(boma, SYNTH_PER_FRAME);
                if PHASE_TIMER[e] % 20 == 0 {
                    effect_follow(boma, "sys_recovery", "top", 0.0, 0.0, 0.0, 0.8);
                    let glow = effect_follow(boma, "sys_smash_flash_s", "flower", 0.0, 0.0, 0.0, 0.4 + 1.2 * CHARGE[e] / CHARGE_MAX);
                    EffectModule::set_rgb(boma, glow as u32, 1.0, 1.0, 0.5);
                }
            }
        }
        Phase::SynthEnd => {
            if PHASE_TIMER[e] >= SYNTH_END_FRAMES {
                to_idle(fighter, false);
            }
        }
        Phase::Beam => {}
    }
    L2CValue::I32(0)
}

// ---- move scripts -------------------------------------------------------------------------------

/// The Bullet Seed loop animation is now only Synthesis' pose: no seed effects or sounds.
unsafe extern "C" fn silent(_agent: &mut L2CAgentBase) {}

/// Up smash: HDR's script, with PM's frame-13 startup (HDR: about frame 21).
unsafe extern "C" fn game_attackhi4(agent: &mut L2CAgentBase) {
    let lua_state = agent.lua_state_agent;
    let boma = agent.module_accessor;
    frame(lua_state, 1.0);
    FT_MOTION_RATE_RANGE(agent, 1.0, 8.0, 3.0);
    frame(lua_state, 8.0);
    FT_MOTION_RATE_RANGE(agent, 8.0, 26.0, 10.0);
    if is_excute(agent) {
        WorkModule::on_flag(boma, *FIGHTER_STATUS_ATTACK_FLAG_START_SMASH_HOLD);
    }
    frame(lua_state, 26.0);
    FT_MOTION_RATE(agent, 1.0);
    if is_excute(agent) {
        ATTACK(agent, 0, 0, Hash40::new("top"), 17.0, 82, 72, 0, 88, 13.0, 0.0, 17.0, 0.0, None, None, None, 1.0, 1.0, *ATTACK_SETOFF_KIND_ON, *ATTACK_LR_CHECK_F, false, 10, 0.0, 0, false, false, false, false, true, *COLLISION_SITUATION_MASK_GA, *COLLISION_CATEGORY_MASK_ALL, *COLLISION_PART_MASK_ALL, false, Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_L, *COLLISION_SOUND_ATTR_FIRE, *ATTACK_REGION_BOMB);
    }
    wait(lua_state, 4.0);
    if is_excute(agent) {
        AttackModule::clear_all(boma);
    }
}

// ---- every frame ----------------------------------------------------------------------------------

unsafe extern "C" fn on_start(fighter: &mut L2CFighterCommon) {
    let e = entry(fighter.module_accessor);
    SEED_MODE[e] = false;
    SWITCH_REQ[e] = false;
    CHARGE[e] = 0.0;
    LIFT_USED[e] = false;
    take_over(fighter);
}

unsafe fn bulb_hits(boma: *mut BattleObjectModuleAccessor, e: usize, status: i32) {
    let hit = AttackModule::is_infliction_status(boma, *COLLISION_KIND_MASK_HIT);
    let new_status = status != PREV_STATUS[e];
    let new_hit = hit && (!PREV_HIT[e] || new_status);
    PREV_HIT[e] = hit;
    PREV_STATUS[e] = status;
    let m = MotionModule::motion_kind(boma);
    if new_status && status == *FIGHTER_STATUS_KIND_THROW && m == hash40("throw_hi") {
        heal_and_charge(boma, HEAL_UP_THROW);
    }
    if !new_hit {
        return;
    }
    let amount = if status == *FIGHTER_STATUS_KIND_ATTACK_HI4 {
        HEAL_UP_SMASH
    } else if status == *FIGHTER_STATUS_KIND_ATTACK_AIR && m == hash40("attack_air_hi") {
        HEAL_UP_AIR
    } else if status == *FIGHTER_STATUS_KIND_ATTACK_AIR && m == hash40("attack_air_lw") {
        HEAL_DOWN_AIR
    } else if status == *FIGHTER_STATUS_KIND_CATCH_ATTACK {
        HEAL_PUMMEL
    } else {
        return;
    };
    heal_and_charge(boma, amount);
}

unsafe extern "C" fn frame_main(fighter: &mut L2CFighterCommon) {
    take_over(fighter);
    let boma = fighter.module_accessor;
    let e = entry(boma);
    let status = StatusModule::status_kind(boma);

    // down taunt -> Pokemon switch
    if status == *FIGHTER_STATUS_KIND_APPEAL && !SWITCH_REQ[e] {
        let m = MotionModule::motion_kind(boma);
        if (m == hash40("appeal_lw_l") || m == hash40("appeal_lw_r"))
            && LinkModule::is_link(boma, *FIGHTER_POKEMON_LINK_NO_PTRAINER)
        {
            SWITCH_REQ[e] = true;
            fighter.change_status(L2CValue::I32(*FIGHTER_STATUS_KIND_SPECIAL_LW), L2CValue::Bool(false));
            return;
        }
    }
    if SWITCH_REQ[e] && status != *FIGHTER_STATUS_KIND_APPEAL && status != *FIGHTER_STATUS_KIND_SPECIAL_LW {
        SWITCH_REQ[e] = false;
    }

    bulb_hits(boma, e, status);

    // full charge: the bulb glows now and then so both players can see it
    if CHARGE[e] >= CHARGE_MAX {
        GLOW_TIMER[e] += 1;
        if GLOW_TIMER[e] % 40 == 0 {
            let glow = effect_follow(boma, "sys_smash_flash_s", "flower", 0.0, 0.0, 0.0, 1.0);
            EffectModule::set_rgb(boma, glow as u32, 1.0, 1.0, 0.5);
        }
    }

    // tether
    let air = StatusModule::situation_kind(boma) == *SITUATION_KIND_AIR;
    if !air || status == *FIGHTER_STATUS_KIND_CLIFF_CATCH {
        LIFT_USED[e] = false;
    }
    if status == *FIGHTER_STATUS_KIND_SPECIAL_HI && !StopModule::is_stop(boma) {
        let f = MotionModule::frame(boma);
        if f < 12.0 {
            MotionModule::set_rate(boma, TETHER_STARTUP_RATE);
        }
        if air && (TETHER_CHECK_FROM..=TETHER_CHECK_TO).contains(&f) {
            WorkModule::on_flag(boma, *FIGHTER_STATUS_AIR_LASSO_FLAG_CHECK);
        }
        if air && !LIFT_USED[e] && f >= 10.0 {
            LIFT_USED[e] = true;
            KineticModule::add_speed(boma, &Vector3f { x: 0.0, y: TETHER_LIFT, z: 0.0 });
        }
    }
}

pub fn install() {
    Agent::new("pfushigisou")
        .acmd("game_attackhi4", game_attackhi4, Priority::High)
        .acmd("effect_specialn", silent, Priority::High)
        .acmd("effect_specialairn", silent, Priority::High)
        .acmd("sound_specialn", silent, Priority::High)
        .acmd("sound_specialairn", silent, Priority::High)
        .on_start(on_start)
        .on_line(Main, frame_main)
        .install();
}
