"""Generates src/vanilla_acmd.rs: real copies of vanilla move scripts (from WuBoytH's
SSBU-Dumped-Scripts, smashline-hdr format) for every script HDR overrides on K. Rool, his
articles, and Dedede's down tilt. Registered at Priority::High so they beat HDR's Priority::Low.

Usage: py gen_vanilla_acmd.py <dump tree json> <hdr-src fighters dir> <out .rs>
"""
import json, re, sys, urllib.request, os, concurrent.futures

tree_path, hdr_fighters, out_path = sys.argv[1:4]
RAW = "https://raw.githubusercontent.com/WuBoytH/SSBU-Dumped-Scripts/main/"
paths = [t["path"] for t in json.load(open(tree_path))["tree"]]

def hdr_names(subdir):
    names = set()
    for root, _, files in os.walk(os.path.join(hdr_fighters, subdir)):
        for f in files:
            if f.endswith(".rs"):
                for line in open(os.path.join(root, f), encoding="utf-8"):
                    if line.strip().startswith("//"):
                        continue
                    names |= set(re.findall(r'\.acmd\("([a-z0-9_]+)"', line))
    return sorted(names)

# (rust module, smashline agent, dump folder, script names)
agents = [
    ("krool", "krool", "smashline-hdr/lua2cpp_krool/krool/", hdr_names("krool/src/acmd")),
    ("krool_backpack", "krool_backpack", "smashline-hdr/lua2cpp_krool/krool_backpack/", hdr_names("krool/src/backpack")),
    ("krool_crown", "krool_crown", "smashline-hdr/lua2cpp_krool/krool_crown/", hdr_names("krool/src/crown")),
    ("krool_ironball", "krool_ironball", "smashline-hdr/lua2cpp_krool/krool_ironball/", hdr_names("krool/src/ironball")),
    ("dedede", "dedede", "smashline-hdr/lua2cpp_dedede/dedede/", ["game_attacklw3", "effect_attacklw3", "expression_attacklw3"]),
]

def fetch(p):
    with urllib.request.urlopen(RAW + p.replace(" ", "%20")) as r:
        return p, r.read().decode("utf-8")

IMPORTS = """use smash::{
    app::{self, lua_bind::*, sv_animcmd::{frame, wait}, *},
    hash40,
    lib::{lua_const::*, L2CValue, LuaConst},
    lua2cpp::*,
    phx::*,
};
use smash_script::{macros::*, *};
use smashline::*;"""


# Hand-written replacements for scripts the decompiler mangled (misplaced else / broken
# L2CValue comparison). Logic taken from the dumped versions.
COUNTER_FLASH = """    let lua_state = agent.lua_state_agent;
    let boma = agent.module_accessor;
    frame(lua_state, 6.0);
    if sv_animcmd::get_value_float(lua_state, *SO_VAR_FLOAT_LR) < 0.0 {
        if is_excute(agent) {
            EFFECT_FOLLOW(agent, Hash40::new("krool_counter_flash_l"), Hash40::new("top"), 0, 0, 0, 0, 0, 0, 1, true);
        }
    } else {
        if is_excute(agent) {
            EFFECT_FOLLOW(agent, Hash40::new("krool_counter_flash"), Hash40::new("top"), 0, 0, 0, 0, 0, 0, 1, true);
        }
    }
"""
CANNON_FIRE_SOUND = """    let lua_state = agent.lua_state_agent;
    let boma = agent.module_accessor;
    frame(lua_state, 1.0);
    if is_excute(agent) {
        PLAY_SE(agent, Hash40::new("se_krool_special_n08"));
    }
    frame(lua_state, 31.0);
    if !WorkModule::is_flag(boma, *FIGHTER_KROOL_STATUS_SPECIAL_N_FLAG_NO_SHOOT_CANCEL) {
        if is_excute(agent) {
            PLAY_SE(agent, Hash40::new("se_krool_special_n07"));
        }
    } else {
        if is_excute(agent) {
            PLAY_SE(agent, Hash40::new("se_krool_special_n01"));
        }
    }
    frame(lua_state, 60.0);
    if is_excute(agent) {
        PLAY_SE(agent, Hash40::new("se_krool_special_n09"));
    }
"""
MANUAL = {
    "effect_speciallw": COUNTER_FLASH,
    "effect_specialairlw": COUNTER_FLASH,
    "sound_specialnfire": CANNON_FIRE_SOUND,
    "sound_specialairnfire": CANNON_FIRE_SOUND,
}

# Decompiler artifact: "CHECK;" followed by "if(methodlib::L2CValue::operator==(...)(false, true)){"
# really means "if CHECK {" ("(false, false)" means "if !CHECK {").
L2C_CMP = re.compile(r"^(\s*)([^\n;]+);\n\s*if\(methodlib::L2CValue::operator==\(lib::L2CValueconst&\)const\(false, (true|false)\)\)\{", re.M)
def fix_l2c_cmp(body):
    return L2C_CMP.sub(lambda m: f"{m.group(1)}if {'' if m.group(3) == 'true' else '!'}{m.group(2).strip()} {{", body)

# Other decompiler artifacts -> what they call.
TEXT_FIXES = [
    ("methodlua2cpp::L2CFighterAnimcmdExpressionCommon::expression_LandingHeavyRumble()();",
     'if is_excute(agent) {\n        ControlModule::set_rumble(boma, Hash40::new("rbkind_landl"), 0, false, *BATTLE_OBJECT_ID_INVALID as u32);\n    }'),
    ("    execute(lua_state,", "    smash::app::sv_animcmd::execute(lua_state,"),
    # the lua_const ATTACK_ABS shadows the smash_script macro of the same name
    ("ATTACK_ABS(agent,", "macros::ATTACK_ABS(agent,"),
]
# integer literal where an f32 is expected, e.g. MotionModule::set_rate(boma, 2)
INT_RATE = re.compile(r"(MotionModule::set_rate\(boma, )(\d+)\)")
# smash_script's SEARCH takes the second point (x2, y2, z2) as Option<f32>; the dumps pass plain floats.
SEARCH = re.compile(r"(SEARCH\(agent, [^,]+, [^,]+, Hash40::new\(\"[^\"]*\"\), [^,]+, [^,]+, [^,]+, [^,]+, )(-?[0-9.]+), (-?[0-9.]+), (-?[0-9.]+),")
# smash_script's ATTACK_IGNORE_THROW has (x2, y2, z2) Options after z; the dumps leave them out.
IGNORE_THROW = re.compile(r"(ATTACK_IGNORE_THROW\(agent, (?:[^,]+, ){12})(?!Some|None)")
# Throw-finish cut-in camera offset: cosmetic, and the singleton accessor isn't in our bindings.
CUTIN = re.compile(r"^.*singletons::FighterCutInManager\(\).*$", re.M)
def fix_text(body):
    for a, b in TEXT_FIXES:
        body = body.replace(a, b)
    body = IGNORE_THROW.sub(r"\g<1>None, None, None, ", body)
    body = CUTIN.sub("        // (vanilla throw cut-in camera offset omitted)", body)
    body = SEARCH.sub(r"\g<1>Some(\g<2>), Some(\g<3>), Some(\g<4>),", body)
    return INT_RATE.sub(r"\g<1>\g<2>.0)", body)

FN = re.compile(r'^unsafe extern "C" fn ([a-z0-9_]+)\(agent: &mut L2CAgentBase\) \{\n(.*?)^\}', re.S | re.M)

out = ["// GENERATED by gen_vanilla_acmd.py - vanilla scripts from WuBoytH/SSBU-Dumped-Scripts.",
       "#![allow(unused)]", IMPORTS, ""]
total = missing = 0
for module, agent, folder, names in agents:
    files = [p for p in paths if p.startswith(folder) and p.endswith(".txt")]
    found = {}
    with concurrent.futures.ThreadPoolExecutor(16) as ex:
        for _, text in ex.map(fetch, files):
            for m in FN.finditer(text):
                # The dumps sometimes have decompiler brace mistakes, so take the body by
                # counting braces from the function's opening brace instead of the regex match.
                start = m.start(2)
                depth, i = 1, start
                while depth and i < len(text):
                    depth += {"{": 1, "}": -1}.get(text[i], 0)
                    i += 1
                body = text[start:i - 1]
                if body != m.group(2):
                    print(f"  brace mismatch in {m.group(1)} (check by hand)")
                found.setdefault(m.group(1), body)
    out.append(f"pub mod {module} {{\n    use super::*;")
    for n in names:
        body = MANUAL.get(n) if module == "krool" and n in MANUAL else found.get(n)
        if body is None:
            missing += 1
            out.append(f"    // {n}: no vanilla script -> empty, so HDR's script doesn't run")
            out.append(f"    pub unsafe extern \"C\" fn {n}(_agent: &mut L2CAgentBase) {{}}")
        else:
            body = fix_text(fix_l2c_cmp(body.replace("agent.boma()", "agent.module_accessor")))
            out.append(f"    pub unsafe extern \"C\" fn {n}(agent: &mut L2CAgentBase) {{\n{body}    }}")
        total += 1
    out.append("    pub fn install() {")
    out.append(f"        let agent = &mut Agent::new(\"{agent}\");")
    for n in names:
        out.append(f"        agent.acmd(\"{n}\", {n}, Priority::High);")
    out.append("        agent.install();\n    }\n}\n")
    print(f"{agent}: {len(names)} scripts, {len([n for n in names if n not in found])} without vanilla version")
open(out_path, "w", encoding="utf-8", newline="\n").write("\n".join(out))
print(f"wrote {total} scripts ({missing} empty) to {out_path}")
