// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/table-borders.ts (paragraph borders subset);
// changes listed in PROVENANCE.md.

use serde_json::{json, Value};

use nitro_xml_opc::xml::Element;

use crate::simple_types::{parse_eighth_points, parse_rgb_color, parse_unsigned_integer};
use crate::xml_util::{first_w, get_w};

pub fn parse_shading_fill(shd: Option<&Element>) -> Option<String> {
    parse_rgb_color(get_w(shd, "fill").as_deref())
}

pub fn border_side(element: Option<&Element>) -> Option<Value> {
    let element = element?;
    let style = get_w(Some(element), "val").filter(|v| is_st_border(v));
    let size = parse_eighth_points(get_w(Some(element), "sz").as_deref());
    let color = parse_rgb_color(get_w(Some(element), "color").as_deref());
    if style.is_none() && size.is_none() && color.is_none() {
        return None;
    }
    let mut obj = serde_json::Map::new();
    if let Some(s) = style {
        obj.insert("style".into(), json!(s));
    }
    if let Some(s) = size {
        obj.insert("sizeEighthPoints".into(), json!(s));
    }
    if let Some(c) = color {
        obj.insert("color".into(), json!(c));
    }
    Some(Value::Object(obj))
}

pub fn parse_paragraph_borders(p_bdr: Option<&Element>) -> Option<Value> {
    let p_bdr = p_bdr?;
    let mut result = serde_json::Map::new();
    for side in ["top", "bottom", "left", "right", "between"] {
        let element = first_w(Some(p_bdr), side).or_else(|| {
            if side == "left" {
                first_w(Some(p_bdr), "start")
            } else if side == "right" {
                first_w(Some(p_bdr), "end")
            } else {
                None
            }
        });
        if let Some(mut value) = border_side(element) {
            if let Some(space) = parse_unsigned_integer(get_w(element, "space").as_deref()) {
                if let Value::Object(ref mut m) = value {
                    m.insert("spacePoints".into(), json!(space));
                }
            }
            result.insert(side.into(), value);
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(Value::Object(result))
    }
}

fn is_st_border(value: &str) -> bool {
    matches!(
        value,
        "nil"
            | "none"
            | "single"
            | "thick"
            | "double"
            | "dotted"
            | "dashed"
            | "dotDash"
            | "dotDotDash"
            | "triple"
            | "thinThickSmallGap"
            | "thickThinSmallGap"
            | "thinThickThinSmallGap"
            | "thinThickMediumGap"
            | "thickThinMediumGap"
            | "thinThickThinMediumGap"
            | "thinThickLargeGap"
            | "thickThinLargeGap"
            | "thinThickThinLargeGap"
            | "wave"
            | "doubleWave"
            | "dashSmallGap"
            | "dashDotStroked"
            | "threeDEmboss"
            | "threeDEngrave"
            | "outset"
            | "inset"
            | "apples"
            | "archedScallops"
            | "babyPacifier"
            | "babyRattle"
            | "balloons3Colors"
            | "balloonsHotAir"
            | "basicBlackDashes"
            | "basicBlackDots"
            | "basicBlackSquares"
            | "basicThinLines"
            | "basicWhiteDashes"
            | "basicWhiteDots"
            | "basicWhiteSquares"
            | "basicWideInline"
            | "basicWideMidline"
            | "basicWideOutline"
            | "bats"
            | "birds"
            | "birdsFlight"
            | "cabins"
            | "cakeSlice"
            | "candyCorn"
            | "celticKnotwork"
            | "certificateBanner"
            | "chainLink"
            | "champagneBottle"
            | "checkedBarBlack"
            | "checkedBarColor"
            | "checkered"
            | "christmasTree"
            | "circlesLines"
            | "circlesRectangles"
            | "classicalWave"
            | "clocks"
            | "compass"
            | "confetti"
            | "confettiGrays"
            | "confettiOutline"
            | "confettiStreamers"
            | "confettiWhite"
            | "cornerTriangles"
            | "couponCutoutDashes"
            | "couponCutoutDots"
            | "crazyMaze"
            | "creaturesButterfly"
            | "creaturesFish"
            | "creaturesInsects"
            | "creaturesLadyBug"
            | "crossStitch"
            | "cup"
            | "decoArch"
            | "decoArchColor"
            | "decoBlocks"
            | "diamondsGray"
            | "doubleD"
            | "doubleDiamonds"
            | "earth1"
            | "earth2"
            | "earth3"
            | "eclipsingSquares1"
            | "eclipsingSquares2"
            | "eggsBlack"
            | "fans"
            | "film"
            | "firecrackers"
            | "flowersBlockPrint"
            | "flowersDaisies"
            | "flowersModern1"
            | "flowersModern2"
            | "flowersPansy"
            | "flowersRedRose"
            | "flowersRoses"
            | "flowersTeacup"
            | "flowersTiny"
            | "gems"
            | "gingerbreadMan"
            | "gradient"
            | "handmade1"
            | "handmade2"
            | "heartBalloon"
            | "heartGray"
            | "hearts"
            | "heebieJeebies"
            | "holly"
            | "houseFunky"
            | "hypnotic"
            | "iceCreamCones"
            | "lightBulb"
            | "lightning1"
            | "lightning2"
            | "mapPins"
            | "mapleLeaf"
            | "mapleMuffins"
            | "marquee"
            | "marqueeToothed"
            | "moons"
            | "mosaic"
            | "musicNotes"
            | "northwest"
            | "ovals"
            | "packages"
            | "palmsBlack"
            | "palmsColor"
            | "paperClips"
            | "papyrus"
            | "partyFavor"
            | "partyGlass"
            | "pencils"
            | "people"
            | "peopleWaving"
            | "poinsettias"
            | "postageStamp"
            | "pumpkin1"
            | "pushPinNote2"
            | "pushPinNote1"
            | "pyramids"
            | "pyramidsAbove"
            | "quadrants"
            | "rings"
            | "safari"
            | "sawtooth"
            | "sawtoothGray"
            | "scaredCat"
            | "seattle"
            | "shadowedSquares"
            | "sharksTeeth"
            | "shorebirdTracks"
            | "skyrocket"
            | "snowflakeFancy"
            | "snowflakes"
            | "southernCross"
            | "stars"
            | "starsTop"
            | "stars3d"
            | "starsBlack"
            | "starsShadowed"
            | "sun"
            | "swirligig"
            | "tornPaper"
            | "tornPaperBlack"
            | "trees"
            | "triangleParty"
            | "triangles"
            | "tribal1"
            | "tribal2"
            | "tribal3"
            | "tribal4"
            | "tribal5"
            | "tribal6"
            | "twistedLines1"
            | "twistedLines2"
            | "vine"
            | "waveline"
            | "weavingAngles"
            | "weavingBraid"
            | "weavingRibbon"
            | "weavingStrips"
            | "whiteFlowers"
            | "woodwork"
            | "xIllusions"
            | "zanyTriangles"
            | "zigZag"
            | "zigZagStitch"
    )
}
