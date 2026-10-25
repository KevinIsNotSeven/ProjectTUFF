//! Ivysaur rework, layered on HDR's Ivysaur:
//! - **Neutral B = Solar Beam.** Hold B to charge (CHARGE_FRAMES to full); keep holding at full
//!   charge to heal (HEAL_PER_FRAME). Let go to fire a light pillar straight up out of the bulb
//!   (Palutena-up-smash style), 8% uncharged up to 22% fully charged. Shield while charging
//!   cancels and stores the charge for next time. Uses the up smash animations; its hitboxes and
//!   pillar are in `game_attackhi4` below (the real up smash is HDR's, unchanged).
//! - **Down B = HDR's vertical seed** (what HDR had on neutral B). One seed out at a time.
//! - **Down taunt = Pokemon switch** (what down B was).
//! - **Tether (up B):** faster startup, ledge-grab window frames 11-32 instead of 12-14, and a
//!   lift the first time it's used in the air (once per airtime).
//!
//! How: smashline status scripts have no priority and HDR's plugin registers last, so the
//! status-table slots are written directly every frame (same trick as K. Rool), keeping HDR's
//! functions to call: SPECIAL_LW pre redirects to SPECIAL_N (seed) unless the switch was asked
//! for from the taunt; SPECIAL_N main is Solar Beam unless it came from down B.
use super::*;
use smash::app::sv_animcmd::{frame, wait};
use smash::hash40;
use smash::lua2cpp::{L2CAgentBase, L2CFighterBase};
use smash::phx::{Hash40, Vector3f};
use smash_script::macros::*;

type CVoid = skyline::libc::c_void;
type StatusFn = unsafe extern "C" fn(&mut L2CFighterCommon) -> L2CValue;

const MAX_ENTRIES: usize = 8;
/// Frames of holding B to reach full charge.
const CHARGE_FRAMES: f32 = 90.0;
/// Shortest hold before letting go fires (a tap still fires an uncharged beam).
const MIN_HOLD_FRAMES: i32 = 6;
/// % healed per frame while holding a full charge (0.1 = 6% per second).
const HEAL_PER_FRAME: f32 = 0.1;
/// Solar Beam damage: BEAM_DAMAGE_MIN uncharged, BEAM_DAMAGE_MAX fully charged.
const BEAM_DAMAGE_MIN: f32 = 8.0;
const BEAM_DAMAGE_MAX: f32 = 22.0;
/// Up B: motion rate during the startup (HDR plays frames 1-12 in 16 frames = 0.75).
const TETHER_STARTUP_RATE: f32 = 1.5;
/// Up B: ledge-grab window (animation frames; HDR: 12-14).
const TETHER_CHECK_FROM: f32 = 11.0;
const TETHER_CHECK_TO: f32 = 32.0;
/// Up B: upward speed added the first time per airtime.
const TETHER_LIFT: f32 = 1.4;

static mut SEED_MODE: [bool; MAX_ENTRIES] = [false; MAX_ENTRIES];
static mut SWITCH_REQ: [bool; MAX_ENTRIES] = [false; MAX_ENTRIES];
static mut CHARGE: [f32; MAX_ENTRIES] = [0.0; MAX_ENTRIES];
static mut STORED: [f32; MAX_ENTRIES] = [0.0; MAX_ENTRIES];
static mut FIRING: [bool; MAX_ENTRIES] = [false; MAX_ENTRIES];
static mut HOLD: [i32; MAX_ENTRIES] = [0; MAX_ENTRIES];
static mut POWER: [f32; MAX_ENTRIES] = [0.0; MAX_ENTRIES];
static mut LIFT_USED: [bool; MAX_ENTRIES] = [false; MAX_ENTRIES];
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
    // HDR blocks neutral B while a seed is out; Solar Beam doesn't need that.
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
    CHARGE[e] = STORED[e];
    STORED[e] = 0.0;
    FIRING[e] = false;
    HOLD[e] = 0;
    set_kinetic(boma);
    MotionModule::change_motion(boma, Hash40::new("attack_hi4_hold"), 0.0, 1.0, false, 0.0, false, false);
    fighter.sub_shift_status_main(L2CValue::Ptr(solar_loop as *const () as _))
}

unsafe extern "C" fn special_n_end(fighter: &mut L2CFighterCommon) -> L2CValue {
    let e = entry(fighter.module_accessor);
    SEED_MODE[e] = false;
    FIRING[e] = false;
    call_saved(SAVED_N_END[e], fighter)
}

// ---- Solar Beam -----------------------------------------------------------------------------------

unsafe fn set_kinetic(boma: *mut BattleObjectModuleAccessor) {
    if StatusModule::situation_kind(boma) == *SITUATION_KIND_GROUND {
        GroundModule::correct(boma, app::GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_GROUND_STOP);
    } else {
        GroundModule::correct(boma, app::GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
        KineticModule::change_kinetic(boma, *FIGHTER_KINETIC_TYPE_FALL);
    }
}

unsafe fn to_idle(fighter: &mut L2CFighterCommon) {
    let kind = if StatusModule::situation_kind(fighter.module_accessor) == *SITUATION_KIND_GROUND {
        *FIGHTER_STATUS_KIND_WAIT
    } else {
        *FIGHTER_STATUS_KIND_FALL
    };
    fighter.change_status(L2CValue::I32(kind), L2CValue::Bool(false));
}

unsafe fn effect_on(boma: *mut BattleObjectModuleAccessor, name: &str, y: f32, scale: f32) {
    let pos = Vector3f { x: 0.0, y, z: 0.0 };
    let zero = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
    EffectModule::req_follow(boma, Hash40::new(name), Hash40::new("top"), &pos, &zero, scale, true, 0, 0, 0, 0, 0, false, false);
}

unsafe extern "C" fn solar_loop(fighter: &mut L2CFighterCommon) -> L2CValue {
    let boma = fighter.module_accessor;
    let e = entry(boma);
    if !StatusModule::is_changing(boma) && StatusModule::is_situation_changed(boma) {
        set_kinetic(boma);
    }
    if FIRING[e] {
        if MotionModule::is_end(boma) {
            to_idle(fighter);
        }
        return L2CValue::I32(0);
    }

    // charging
    HOLD[e] += 1;
    if ControlModule::check_button_trigger(boma, *CONTROL_PAD_BUTTON_GUARD) {
        STORED[e] = CHARGE[e]; // keep the charge for next time
        to_idle(fighter);
        return L2CValue::I32(0);
    }
    let holding = ControlModule::check_button_on(boma, *CONTROL_PAD_BUTTON_SPECIAL);
    if CHARGE[e] < CHARGE_FRAMES {
        CHARGE[e] += 1.0;
        if HOLD[e] % 15 == 0 {
            effect_on(boma, "sys_smash_flash", 10.0, 0.6 + CHARGE[e] / CHARGE_FRAMES);
        }
        if CHARGE[e] >= CHARGE_FRAMES {
            effect_on(boma, "sys_level_up", 0.0, 1.0); // full
        }
    } else if holding {
        // full: holding heals
        let dmg = DamageModule::damage(boma, 0);
        if dmg > 0.0 {
            DamageModule::add_damage(boma, -HEAL_PER_FRAME.min(dmg), 0);
        }
        if HOLD[e] % 30 == 0 {
            effect_on(boma, "sys_recovery", 0.0, 1.0);
            effect_on(boma, "sys_smash_flash", 10.0, 1.6);
        }
    }
    if !holding && HOLD[e] >= MIN_HOLD_FRAMES {
        POWER[e] = (CHARGE[e] / CHARGE_FRAMES).clamp(0.0, 1.0);
        CHARGE[e] = 0.0;
        FIRING[e] = true;
        // the up smash from just after its windup; game_attackhi4 sees SPECIAL_N and fires the beam
        MotionModule::change_motion(boma, Hash40::new("attack_hi4"), 8.0, 1.0, false, 0.0, false, false);
    }
    L2CValue::I32(0)
}

unsafe fn solar_beam(agent: &mut L2CAgentBase) {
    let lua_state = agent.lua_state_agent;
    let boma = agent.module_accessor;
    let p = POWER[entry(boma)];
    let dmg = BEAM_DAMAGE_MIN + (BEAM_DAMAGE_MAX - BEAM_DAMAGE_MIN) * p;
    let bkb = (45.0 + 35.0 * p) as i32;
    frame(lua_state, 8.0);
    FT_MOTION_RATE(agent, 0.6);
    frame(lua_state, 24.0);
    if is_excute(agent) {
        EFFECT(agent, Hash40::new("sys_smash_flash"), Hash40::new("top"), 0, 12, 0, 0, 0, 0, 1.0 + p, 0, 0, 0, 0, 0, 0, false);
    }
    frame(lua_state, 26.0);
    FT_MOTION_RATE(agent, 1.0);
    if is_excute(agent) {
        // the pillar: base (strongest) and upper column
        ATTACK(agent, 0, 0, Hash40::new("top"), dmg, 88, 80, 0, bkb, 10.0, 0.0, 9.0, 0.0, Some(0.0), Some(30.0), Some(0.0), 1.0 + 0.5 * p, 1.0, *ATTACK_SETOFF_KIND_ON, *ATTACK_LR_CHECK_POS, false, 0, 0.0, 0, false, false, false, false, true, *COLLISION_SITUATION_MASK_GA, *COLLISION_CATEGORY_MASK_ALL, *COLLISION_PART_MASK_ALL, false, Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_L, *COLLISION_SOUND_ATTR_FIRE, *ATTACK_REGION_BOMB);
        ATTACK(agent, 1, 0, Hash40::new("top"), dmg * 0.75, 85, 80, 0, bkb, 8.5, 0.0, 30.0, 0.0, Some(0.0), Some(75.0), Some(0.0), 1.0, 1.0, *ATTACK_SETOFF_KIND_ON, *ATTACK_LR_CHECK_POS, false, 0, 0.0, 0, false, false, false, false, true, *COLLISION_SITUATION_MASK_GA, *COLLISION_CATEGORY_MASK_ALL, *COLLISION_PART_MASK_ALL, false, Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_L, *COLLISION_SOUND_ATTR_FIRE, *ATTACK_REGION_BOMB);
        let scale = 0.9 + 0.5 * p;
        for i in 0..6 {
            EFFECT(agent, Hash40::new("pfushigisou_atk_hi4"), Hash40::new("top"), 0, 6.0 + 12.0 * i as f32, 0, 0, 0, 0, scale, 0, 0, 0, 0, 0, 0, false);
            LAST_EFFECT_SET_COLOR(agent, 0.8, 1.0, 0.6);
        }
        EFFECT(agent, Hash40::new("sys_smash_flash"), Hash40::new("top"), 0, 40, 0, 0, 0, 0, 2.0 + p, 0, 0, 0, 0, 0, 0, false);
        PLAY_SE(agent, Hash40::new("se_common_bomb_l"));
        QUAKE(agent, if p >= 1.0 { *CAMERA_QUAKE_KIND_L } else { *CAMERA_QUAKE_KIND_M });
    }
    wait(lua_state, 6.0);
    if is_excute(agent) {
        AttackModule::clear_all(boma);
    }
}

/// Up smash: HDR's script, except when it's Solar Beam (played from neutral B).
unsafe extern "C" fn game_attackhi4(agent: &mut L2CAgentBase) {
    let lua_state = agent.lua_state_agent;
    let boma = agent.module_accessor;
    if StatusModule::status_kind(boma) == *FIGHTER_STATUS_KIND_SPECIAL_N {
        solar_beam(agent);
        return;
    }
    // HDR v0.50.0 fighters/pfushigisou/src/acmd/smashes.rs game_attackhi4
    frame(lua_state, 1.0);
    FT_MOTION_RATE_RANGE(agent, 1.0, 8.0, 3.0);
    frame(lua_state, 8.0);
    FT_MOTION_RATE(agent, 1.0);
    if is_excute(agent) {
        WorkModule::on_flag(boma, *FIGHTER_STATUS_ATTACK_FLAG_START_SMASH_HOLD);
    }
    frame(lua_state, 26.0);
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
    STORED[e] = 0.0;
    LIFT_USED[e] = false;
    take_over(fighter);
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
        .on_start(on_start)
        .on_line(Main, frame_main)
        .install();
}
