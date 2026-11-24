//! Generated workspace names: marine creatures, the pool the reference app draws from (ADR 0008).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

/// The pool, lowercase and whitespace separated. A name of tier N past the first is `name-N`.
const CREATURES: &str = "
    nautilus seahorse starfish coral narwhal jellyfish octopus manta dolphin manatee
    cuttlefish anemone urchin triton nudibranch coelacanth oarfish beluga dugong porpoise
    stingray hammerhead wobbegong sawfish chimaera anglerfish viperfish dragonfish axolotl
    otter seadragon seahare squid argonaut sponge barnacle cowrie guitarfish lamprey isopod
    lionfish clownfish angelfish butterflyfish parrotfish pufferfish moonfish firefish
    unicornfish rainbowfish betta discus arowana koi piranha barracuda moray sunfish
    lanternfish archerfish mudskipper hatchetfish knifefish leaffish glassfish ropefish
    bichir tigerfish cardinalfish lungfish opah frogfish stonefish cutlassfish paddlefish
    arapaima mandarin blobfish thresher vaquita pipefish guppy tetra danio cichlid oscar
    gourami killifish rasbora pleco goldfish molly platy barb loach medaka pacu filefish
    boxfish cowfish surgeonfish damselfish wrasse goby blenny conger sculpin darter remora
    pilotfish trumpetfish cornetfish jawfish toadfish pearlfish driftfish lumpfish
    snailfish stickleback halfbeak snipefish pencilfish snakehead tripletail lookdown
    sweetlips squirrelfish soldierfish rabbitfish hawkfish bannerfish batfish chromis
    anthias fusilier sweeper emperor ponyfish kelpfish weever pearlside opaleye ballyhoo
    needlefish triggerfish gar tarpon sailfish conch walrus seal penguin hagfish gulper
    hydra krill salp tunicate crinoid polyp limpet whelk bowfin minnow gudgeon goldeye
    mooneye soapfish leatherjacket rudderfish sabrefish puffin albatross osprey pelican
    petrel gannet tern cormorant leatherback hawksbill loggerhead dottyback basslet gramma
    hamlet hogfish dartfish foxface shrimpfish pyrosome seapen medusa candlefish silverside
    shiner eelpout madtom shovelnose sandlance tubesnout marlin bonefish dace rudd spirula
    permit grunt trevally croaker drum ladyfish bullhead torpedo skate hermit fiddler
    fireworm tubeworm amphipod copepod pteropod seaslug swordtail bluegill pickerel
    redhorse logperch goblin catshark harlequin murex volute wentletrap nerite stargazer
    handfish flyingfish waspfish skua shearwater guillemot razorbill auk fulmar murre
    velvetfish prowfish sandperch bobtail chiton featherstar nereid siren kraken selkie
    leviathan wolffish wreckfish horseshoe orca cachalot rorqual grampus humpback bowhead
    finback sealion pinniped cetacean mako porbeagle dogfish spurdog tope bonnethead
    sixgill sevengill angelshark houndshark megamouth cookiecutter bramble ghostshark
    sandtiger galeocerdo nursehound cownose devilray eagleray butterflyray numbfish
    stingaree mantaray tuna albacore bonito skipjack wahoo kingfish mackerel cero dorado
    mahimahi escolar pomfret butterfish cod haddock pollock whiting hake ling cusk burbot
    saithe sablefish lingcod greenling rockling halibut flounder sole plaice turbot brill
    dab fluke megrim sanddab herring sardine anchovy sprat pilchard menhaden shad alewife
    salmon trout char grayling steelhead kokanee chinook coho sockeye taimen huchen cisco
    inconnu vendace whitefish perch bass snapper grouper seabass tilefish bream porgy
    sheepshead pinfish scup tautog sander zander walleye sauger ruffe comber hind coney
    graysby margate pompano amberjack yellowtail scad runner mullet goatfish threadfin
    snook barramundi milkfish mojarra weakfish corbina queenfish kahawai tilapia cunner
    tuskfish razorfish tang porcupinefish burrfish eel wolfeel gardeneel ribboneel cuskeel
    catfish cory ratfish elephantfish goosefish monkfish sargassum coffinfish seadevil
    scorpionfish rockfish cabezon rosefish redfish bocaccio thornyhead gurnard searobin
    fangtooth bristlemouth barreleye spookfish telescopefish lancetfish tripodfish
    spiderfish daggertooth pearleye ridgehead snaggletooth sturgeon sterlet kaluga cockle
    mussel clam oyster scallop abalone periwinkle cerith turban tellin geoduck quahog
    piddock shipworm lobster crayfish shrimp prawn langostino langouste yabby marron
    dungeness seafan staghorn elkhorn hydroid zoanthid gorgonian seanettle moonjelly
    sanddollar brittlestar basketstar sunstar seacucumber seabiscuit hearturchin
    bristleworm featherduster lugworm sandworm palolo arrowworm acornworm flatworm
    ribbonworm booby frigatebird tropicbird noddy kittiwake dovekie shag anhinga skimmer
    jaeger oystercatcher eider scoter merganser brant ridley flatback terrapin seasnake
    diatom plankton larvacean doliolid kelp seagrass eelgrass dulse bladderwrack mola
    scylla charybdis cetus proteus glaucus hippocamp jormungandr hafgufa kelpie merrow
    nuckelavee afanc rusalka vodyanoy umibozu isonade ningyo mizuchi naga makara bunyip
    taniwha marakihau lusca undine nixie melusine ondine tiamat dagon aspidochelone
    capricorn merfolk merman mermaid timingila bakekujira jiaolong encantado hraesvelg
";

/// The highest tier tried; the pool of 552 runs out long before it.
const MAX_TIER: usize = 999;

/// The first name `is_taken` does not refuse, at the lowest tier with a free name. `choice`
/// picks among that tier's free names.
pub(crate) fn generated_name(is_taken: impl Fn(&str) -> bool, choice: usize) -> Option<String> {
    (1..=MAX_TIER).find_map(|tier| {
        let mut free: Vec<String> = CREATURES
            .split_whitespace()
            .map(|creature| {
                if tier == 1 {
                    creature.to_owned()
                } else {
                    format!("{creature}-{tier}")
                }
            })
            .filter(|name| !is_taken(name))
            .collect();
        (!free.is_empty()).then(|| free.swap_remove(choice % free.len()))
    })
}

/// A number to pick a name with.
pub(crate) fn random_choice() -> usize {
    // Why: each RandomState is seeded afresh, so an empty hash differs from call to call.
    RandomState::new().build_hasher().finish() as usize
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{CREATURES, generated_name};
    use crate::names::workspace_name;

    #[test]
    fn the_pool_has_distinct_lowercase_names_that_are_valid_workspace_names() {
        let names: Vec<&str> = CREATURES.split_whitespace().collect();
        let distinct: HashSet<&str> = names.iter().copied().collect();
        assert_eq!(distinct.len(), names.len(), "the pool repeats a name");
        for name in names {
            assert!(name.bytes().all(|byte| byte.is_ascii_lowercase()), "{name}");
            workspace_name(name).expect(name);
        }
    }

    #[test]
    fn the_choice_picks_among_the_free_creatures_in_pool_order() {
        assert_eq!(generated_name(|_| false, 0).as_deref(), Some("nautilus"));
        assert_eq!(generated_name(|_| false, 1).as_deref(), Some("seahorse"));
    }

    #[test]
    fn a_taken_creature_is_never_picked() {
        for choice in 0..60 {
            let name =
                generated_name(|candidate| candidate == "nautilus", choice).expect("a free name");
            assert_ne!(name, "nautilus");
        }
    }

    #[test]
    fn once_every_creature_is_taken_the_next_tier_is_used() {
        let name = generated_name(|candidate| !candidate.contains('-'), 0).expect("a free name");
        assert_eq!(name, "nautilus-2");
    }
}
