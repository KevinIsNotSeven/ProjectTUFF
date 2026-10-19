use super::*;

// Copied from the vanilla game scripts (WuBoytH/SSBU-Dumped-Scripts), then edited.

// MELEE: laser 3% for the first 9 frames, then 2% (vanilla: 3% -> 2% after 2 frames -> 1.4% after 9). Still flinchless.
unsafe extern "C" fn game_fly(agent: &mut L2CAgentBase) {
    let lua_state = agent.lua_state_agent;
    let boma = agent.module_accessor;
    if is_excute(agent) {
        ATTACK(agent, 0, 0, Hash40::new("top"), 3.0, 361, 0, 0, 0, 1.44, 0.0, 0.0, 0.8, Some(0.0), Some(0.0), Some(9.0), 1.0, 1.0, *ATTACK_SETOFF_KIND_OFF, *ATTACK_LR_CHECK_SPEED, false, 0, 0.0, 0, true, true, false, false, false, *COLLISION_SITUATION_MASK_GA, *COLLISION_CATEGORY_MASK_ALL, *COLLISION_PART_MASK_ALL, false, Hash40::new("collision_attr_blaster_throw_down"), *ATTACK_SOUND_LEVEL_M, *COLLISION_SOUND_ATTR_FOX_BLASTER, *ATTACK_REGION_ENERGY);
    }
    wait(lua_state, 9.0);
    if is_excute(agent) {
        ATK_POWER(agent, 0, 2);
    }
}

// MELEE: Illusion 7% on the ground and in the air (vanilla: 8% ground / 5% air).
unsafe extern "C" fn game_moveground(agent: &mut L2CAgentBase) {
    let lua_state = agent.lua_state_agent;
    let boma = agent.module_accessor;
    if is_excute(agent) {
        ATTACK(agent, 0, 0, Hash40::new("top"), 7.0, 85, 110, 0, 35, 4.0, 0.0, 4.0, 0.0, None, None, None, 1.0, 1.0, *ATTACK_SETOFF_KIND_ON, *ATTACK_LR_CHECK_F, false, 0, 0.0, 0, false, false, false, false, true, *COLLISION_SITUATION_MASK_GA, *COLLISION_CATEGORY_MASK_ALL, *COLLISION_PART_MASK_ALL, false, Hash40::new("collision_attr_cutup"), *ATTACK_SOUND_LEVEL_M, *COLLISION_SOUND_ATTR_CUTUP, *ATTACK_REGION_BODY);
    }
}

unsafe extern "C" fn game_moveair(agent: &mut L2CAgentBase) {
    let lua_state = agent.lua_state_agent;
    let boma = agent.module_accessor;
    if is_excute(agent) {
        ATTACK(agent, 0, 0, Hash40::new("top"), 7.0, 60, 148, 0, 15, 4.0, 0.0, 4.0, 0.0, None, None, None, 1.0, 1.0, *ATTACK_SETOFF_KIND_ON, *ATTACK_LR_CHECK_F, false, 0, 0.0, 0, false, false, false, false, true, *COLLISION_SITUATION_MASK_GA, *COLLISION_CATEGORY_MASK_ALL, *COLLISION_PART_MASK_ALL, false, Hash40::new("collision_attr_cutup"), *ATTACK_SOUND_LEVEL_M, *COLLISION_SOUND_ATTR_CUTUP, *ATTACK_REGION_BODY);
    }
}

pub fn install_blaster_bullet(agent: &mut Agent) {
    agent.acmd("game_fly", game_fly, Priority::High);
}

pub fn install_illusion(agent: &mut Agent) {
    agent.acmd("game_moveground", game_moveground, Priority::High);
    agent.acmd("game_moveair", game_moveair, Priority::High);
}
