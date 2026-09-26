// Generator is a large C struct; Rust only ever holds it behind this pointer.
#include <stdlib.h>
#include <string.h>
#include "generator.h"
#include "finders.h"

Generator *bmr_generator_new(int mc)
{
    Generator *g = malloc(sizeof(Generator));
    if (g)
        setupGenerator(g, mc, 0);
    return g;
}

void bmr_generator_free(Generator *g)
{
    free(g);
}

void bmr_apply_overworld_seed(Generator *g, uint64_t seed)
{
    applySeed(g, DIM_OVERWORLD, seed);
}

// structure_set name → StructureType; -1 for sets without an overworld biome check
int bmr_structure_type(const char *set)
{
    static const struct { const char *set; int type; } map[] = {
        {"desert_pyramids", Desert_Pyramid}, {"jungle_temples", Jungle_Temple}, {"swamp_huts", Swamp_Hut},
        {"igloos", Igloo}, {"villages", Village}, {"ocean_ruins", Ocean_Ruin}, {"shipwrecks", Shipwreck},
        {"ocean_monuments", Monument}, {"woodland_mansions", Mansion}, {"pillager_outposts", Outpost},
        {"ruined_portals", Ruined_Portal}, {"ancient_cities", Ancient_City}, {"trail_ruins", Trail_Ruins},
        {"trial_chambers", Trial_Chambers}, {"abandoned_camp", Abandoned_Camp},
    };
    for (size_t i = 0; i < sizeof(map) / sizeof(*map); i++)
        if (!strcmp(map[i].set, set))
            return map[i].type;
    return -1;
}
