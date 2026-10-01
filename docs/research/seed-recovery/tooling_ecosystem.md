# Minecraft Java seed-cracking tooling ecosystem (for a Rust, Java-free cracker)

Research date: 2026-09-26. Dates come from GitHub/crates.io API responses fetched that day.

## cubiomes (C) and its Rust bindings: features, versions, license, maintenance

### Takeaway
Upstream Cubitect/cubiomes (MIT) has been dormant since Nov 2024 and stops at 1.21 "Winter Drop". The **xpple/cubiomes** fork (MIT) is the maintained one: it supports 26.3 and adds terrain generation (1.14+), carvers, ores/ore veins and loot. The Rust crate `cubiomes`/`cubiomes-sys` (villevilli, MIT) wraps upstream and is young and lightly used. It is probably easier to own a small bindgen `-sys` crate over the xpple fork.

### Cited Findings
- Upstream cubiomes is MIT-licensed. It "mimics the biome and feature generation of Minecraft Java Edition" and provides structure finding (outposts, swamp huts, quad-witch-huts), stronghold and spawn location, and fast range biome generation — [Cubitect/cubiomes](https://github.com/Cubitect/cubiomes)
- Upstream limitation: "lacks block-level world generation" and so cannot verify surface heights for structures like desert pyramids and jungle temples in 1.18+, which can give false positives — [Cubitect/cubiomes](https://github.com/Cubitect/cubiomes)
- Upstream MCVersion enum ends at `MC_1_19_4`, `MC_1_20_6`, `MC_1_21_1`, `MC_1_21_3`, `MC_1_21_WD` (Winter Drop), `MC_NEWEST = MC_1_21`. The header says "Development effort focuses on just the newest patch for each major release" — [biomes.h](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomes.h)
- The last upstream commit was 2024-11-10 ("Renamed MC_1_21_2 to MC_1_21_3"), and pushed_at is also 2024-11-10. The repo has 957 stars and 55 open issues — [GitHub API commits](https://api.github.com/repos/Cubitect/cubiomes/commits?per_page=5), [GitHub API repo](https://api.github.com/repos/Cubitect/cubiomes)
- The cubiomes-viewer issue #390 "Update to 26.1" (opened 2026-05-04) is still open with no maintainer reply — [cubiomes-viewer #390](https://github.com/Cubitect/cubiomes-viewer/issues/390)
- xpple/cubiomes describes itself as an "Active fork of Cubitect/cubiomes". It is MIT, was created 2024-10-30, was last pushed 2026-09-22 and has 65 stars — [GitHub API](https://api.github.com/repos/xpple/cubiomes)
- Recent xpple commits: "Add 26.3 support (#29)" on 2026-09-11, "Implement pre 1.18 terrain generation (#38)" on 2026-09-22, and a ctest testing framework on 2026-08-28 — [GitHub API commits](https://api.github.com/repos/xpple/cubiomes/commits?per_page=8)
- xpple fork additions: "Up-to-date biome generation", "Terrain generation (1.14+)", canyon/cave carvers (1.13+), ore generation (1.13+) and ore veins (1.18+), strongholds (1.8+), structure loot (1.13+) and "Fast Xoroshiro128++ state advancement" — [xpple/cubiomes README](https://github.com/xpple/cubiomes)
- Rust `cubiomes` crate: v0.3.3, MIT, created 2025-02-16, last updated 2025-03-07, about 5.8k total downloads. The repo is villevilli/cubiomes-rs. It has a `cc_build` feature that compiles C through `cubiomes-sys` — [crates.io API](https://crates.io/api/v1/crates/cubiomes)
- The project is split into cubiomes-sys (bindgen bindings) and cubiomes, which calls itself "(hopefully) safe" — [villevilli/cubiomes-rs](https://github.com/villevilli/cubiomes-rs), [docs.rs](https://docs.rs/cubiomes). A separate EnderKill98/cubiomes-rs also exists — [EnderKill98/cubiomes-rs](https://github.com/EnderKill98/cubiomes-rs/blob/main/README.md)
- mcseedmap.net (which uses cubiomes) says it added Java 26.2 and 26.3 support, so maintained cubiomes derivatives do track 26.x — [mcseedmap.net search result](https://mcseedmap.net/1.21.5-Java/)

### Inferences
- The villevilli crate wraps upstream (dormant) cubiomes, so it probably lacks 1.21.4+ and 26.x. The xpple fork is plain C with a CMake/ctest build, so building it with a project-owned `cc` + `bindgen` `-sys` crate on Windows (MSVC or clang) should be straightforward. This last point was not verified on MSVC.
- xpple's terrain generation (1.14+) gives a noise-based surface height. That is directly useful for matching a BlueMap heightmap, and it closes upstream's "no block-level generation" gap.
- MIT licensing on both upstream and the fork makes vendoring into a Rust workspace unproblematic.

### Gaps
- I did not find the exact per-version accuracy of xpple's terrain and biomes for 26.x, or whether surface rules (not just noise height) are implemented.
- I did not check whether villevilli/cubiomes-rs pins the upstream or the xpple submodule. Its last crates.io release (Mar 2025) predates 26.x.

## SeedcrackerX and the KaptainWutax/SeedFinding Java libraries (LattiCG etc.)

### Takeaway
SeedcrackerX is an actively maintained, MIT-licensed Fabric mod up to MC 26.1/26.2. It runs a two-stage crack: first the 48-bit structure seed from structure positions, decorators and end pillars, then the 64-bit world seed from biomes or the hashed seed. It is Java and in-game only, but its algorithm is the reference design. LattiCG (Java, MIT) already has Rust ports.

### Cited Findings
- SeedcrackerX collects igloos, desert pyramids, jungle temples, swamp huts, shipwrecks, pillager outposts, ocean monuments, end cities, end gateways, desert wells, emerald ore and warped fungus — [SeedcrackerX](https://github.com/19MisterX98/SeedcrackerX)
- For 1.18+ it needs "40 bits from liftable structures OR 32 bits from regular structures". It produces a 48-bit structure seed, then derives the world seed "via dungeon positions or hashed seed brute-forcing" — [SeedcrackerX README](https://github.com/19MisterX98/SeedcrackerX/blob/master/README.md)
- In the End it uses 5+ end cities plus the pillar seed from the dimension centre. It does not crack the Nether and points users to Nether_Bedrock_Cracker — [SeedcrackerX README](https://github.com/19MisterX98/SeedcrackerX/blob/master/README.md)
- Releases: 2.16.0 "26.1" on 2026-03-26, 2.15.6 (1.21.11) on 2025-12-16, 2.15.5 (1.21.9–1.21.10) on 2025-12-10, 2.15.4 (1.21.6–1.21.8) on 2025-08-17, and a prerelease on 2026-07-12 ("Fix BuriedTreasureFinder…") — [GitHub API releases](https://api.github.com/repos/19MisterX98/SeedcrackerX/releases?per_page=5)
- The repo page states versions 1.16.5 through 26.2 (2.16.1 prerelease targets 26.2) and an MIT license — [SeedcrackerX](https://github.com/19MisterX98/SeedcrackerX)
- The original KaptainWutax/SeedCracker is the "Fast, Automatic In-Game Seed Cracker" that SeedcrackerX descends from — [KaptainWutax/SeedCracker](https://github.com/kaptainwutax/seedcracker)
- LattiCG (mjtb49) is Java and MIT. It "reverses the possible internal seed(s) of Java's java.util.Random class given information on its output in the form of a system of inequalities", using lattice reduction plus branch-and-bound. Maven coordinate: `com.seedfinding:latticg:1.07` — [mjtb49/LattiCG](https://github.com/mjtb49/LattiCG). An earlier version exists as Earthcomputer/JavaRandomReverser — [JavaRandomReverser](https://github.com/Earthcomputer/JavaRandomReverser)
- **rusticg** is a Rust rewrite of LattiCG: v1.0.1, Apache-2.0, created 2025-08-30, updated 2025-10-07, 642 downloads, repo L3g73/RustiCG. "Currently only Java's java.util.Random class is implemented" — [crates.io API](https://crates.io/api/v1/crates/rusticg), [docs.rs](https://docs.rs/rusticg/latest/rusticg/)
- SeedFinding/fnseedc collects seed-cracking resources — [fnseedc](https://github.com/SeedFinding/fnseedc). hube12's seed-cracking gist is another primer — [gist](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c)

### Inferences
- The structure-seed stage (region-position LCG constraints) maps well onto rusticg and needs no Java. The world-seed stage (2^16 upper bits per structure seed) needs a correct 1.18+ biome generator, which is cubiomes.
- SeedcrackerX's "dungeon positions" / decorator path depends on carver and feature order, which differs across versions. A BlueMap source gives only surface blocks, so structure positions plus biomes plus heightmap are the realistic inputs.

### Gaps
- I did not verify current maintenance of the SeedFinding Java libs (mc_core, mc_seed, mc_feature…). Their GitHub org paths returned 404 (SeedFinding/LattiCG), so repos may have moved or been archived.
- I did not benchmark rusticg against LattiCG.

## Bedrock crackers

### Takeaway
The main Java-edition bedrock cracker is 19MisterX98's Nether_Bedrock_Cracker. It is Rust and LGPL-3.0, Nether only, 1.18+, and was last pushed Mar 2025. I found no established overworld-bedrock world-seed cracker.

### Cited Findings
- Nether_Bedrock_Cracker: Rust, LGPL-3.0, created 2023-01-12, last push 2025-03-11, 147 stars, default branch `gui` — [GitHub API](https://api.github.com/repos/19MisterX98/Nether_Bedrock_Cracker)
- It works on "1.18 and above since bedrock became seed dependent in that release". It covers the Nether only: "The overworld uses a different RNG, so the calculations used here are not applicable". Collecting both floor and ceiling is advised, since one side gives "drastically more results". Known issue with Paper servers before 1.19.2-213 — [Nether_Bedrock_Cracker](https://github.com/19MisterX98/Nether_Bedrock_Cracker)
- xpple/NetherBedrockCracker is an in-game Fabric mod wrapping the same idea — [xpple/NetherBedrockCracker](https://github.com/xpple/NetherBedrockCracker)
- MiranCZ/BedrockSeedCracker exists; I did not read its README — [MiranCZ/BedrockSeedCracker](https://github.com/MiranCZ/BedrockSeedCracker/blob/master/README.md)
- mc-locate's Nether bedrock crack takes about 1 day on 8 cores for the full 2^48 space — [mc-locate README](https://raw.githubusercontent.com/LAOUUUUU/mc-locate/main/README.md)

### Inferences
- LGPL-3.0 allows linking from a differently licensed Rust binary. Copying its source into an MIT crate would carry LGPL obligations, so a clean reimplementation or dynamic use is safer.
- BlueMap renders the Nether only if that map is configured, and it usually shows the roof, not the floor bedrock. Bedrock is likely a secondary signal for this project.

### Gaps
- Overworld 1.18+ bedrock uses a positional Xoroshiro derived from the full 64-bit seed. I found no public cracker for it and cannot confirm feasibility or complexity.
- There are no published performance figures for Nether_Bedrock_Cracker itself.

## Rust crates for Java Random / Xoroshiro / worldgen, and GPU crackers

### Takeaway
Plenty of basic RNG crates exist. There is no mature, maintained pure-Rust Minecraft 1.18+ worldgen crate that I could verify, so biomes and terrain in practice mean FFI to cubiomes. GPU cracking (Minecraft@Home, KaptainWutax CUDA work) is historical and targeted at specific problems, with no reusable library.

### Cited Findings
- `java-rand` ("implementation of java.util.Random") and `javarandom` ("pure Rust implementation of java.util.Random") are on crates.io — [java-rand](https://crates.io/crates/java-rand), [javarandom](https://crates.io/crates/javarandom)
- `rand_xoshiro` implements xoshiro, xoroshiro and splitmix64 — [rand_xoshiro](https://crates.io/crates/rand_xoshiro/)
- The xpple cubiomes fork includes fast Xoroshiro128++ state advancement — [xpple/cubiomes](https://github.com/xpple/cubiomes)
- Minecraft@Home and the pack.png project found seed 3257840388504953787 on 2020-09-05. They used BOINC distributed computing and, per PC Gamer, NVIDIA DGX-2 machines. The result is valid only for Alpha 1.2.2a–Beta 1.7.3 — [Minecraft@Home pack.png](https://minecraftathome.com/projects/packpng.html), [PC Gamer](https://www.pcgamer.com/the-iconic-minecraft-world-of-the-packpng-image-has-been-found/), [packpng.com](https://packpng.com/)
- KaptainWutax wrote CUDA seed-search programs, such as a tallest-cactus search — [KaptainWutax GitHub](https://github.com/KaptainWutax)

### Inferences
- Minecraft uses Xoroshiro128++ with its own seed mixing and positional forking (hash-based per-feature splitting). This is from general knowledge, not verified this session. A generic `rand_xoshiro` gives only the core step, so the mixing and forking layer has to be written or taken from cubiomes (`rng.h`).
- The 2^48 structure-seed searches in this domain run for minutes to a day on a CPU (see mc-locate). GPU is optional unless constraints are weak.

### Gaps
- I did not check Valence or other Rust server projects for worldgen code. I found no evidence of a 1.18+ vanilla-accurate Rust worldgen crate.
- I did not verify maintenance dates of java-rand, javarandom and xoroshiro crates.

## Tools that crack seeds from maps, screenshots or heightmaps

### Takeaway
**mc-locate** (Rust, 2024 edition) is the closest analogue: multi-source cracking, a Rust port of LattiCG, the xpple cubiomes fork, 26.2 support and no Java. It is very new (created 2026-08-22, 1 star) and its license metadata is inconsistent, so treat it as a design reference to audit, not a trusted dependency. I found no tool that cracks seeds directly from a Dynmap or BlueMap export.

### Cited Findings
- mc-locate (LAOUUUUU): inputs are "bedrock patterns, slime chunks, villages, eye-of-ender throws, F3 screenshots, or chat logs", and it outputs seeds and coordinates. It is Rust (2024 edition), and its README says MIT and versions "Beta 1.7 through 26.2", with Windows x86_64 binaries — [mc-locate](https://github.com/LAOUUUUU/mc-locate)
- Its dependencies are cubiomes, LattiCG ("ported to Rust, not used via JVM", with exact rational arithmetic and Fincke–Pohst enumeration), optional Tesseract OCR and num-bigint/num-rational — [mc-locate](https://github.com/LAOUUUUU/mc-locate), [README](https://raw.githubusercontent.com/LAOUUUUU/mc-locate/main/README.md)
- It uses `xpple/cubiomes` because upstream is "dormant". Generation-dependent modes refuse versions newer than 26.2 rather than substituting a nearby version — [README](https://raw.githubusercontent.com/LAOUUUUU/mc-locate/main/README.md)
- Its method: end pillar heights give the pillar seed (65,536 possibilities), from which it enumerates 2^32 structure seeds (minutes). "Biome observations at the end separate the 65,536 world seeds per structure seed". Slime-chunk full-space search at 2^48 takes about 1 day on 8 cores — [README](https://raw.githubusercontent.com/LAOUUUUU/mc-locate/main/README.md)
- Its heightmap mode uses "cubiomes' approximate surface estimate" with a tolerance parameter rather than full terrain — [README](https://raw.githubusercontent.com/LAOUUUUU/mc-locate/main/README.md)
- Repo metadata: created 2026-08-22, last push 2026-08-30, 1 star, 0 forks, license "Other (NOASSERTION)" — which conflicts with the README's MIT claim — [GitHub API](https://api.github.com/repos/LAOUUUUU/mc-locate)
- Alist2930/MCBE-seedcracker cracks *Bedrock Edition* seeds (1.18–26.3x) from structures and biome samples, so it does not apply to Java — [MCBE-seedcracker](https://github.com/Alist2930/MCBE-seedcracker)
- pack.png, from a 128×128 screenshot, is the canonical "seed from an image" effort. It needed massive distributed computing on pre-1.18 terrain — [Minecraft@Home](https://minecraftathome.com/projects/packpng.html)

### Inferences
- A BlueMap source gives far more data than a screenshot: exact structure positions, dense biome samples and a full heightmap. The standard pipeline is enough: the structure seed via lattice/LCG constraints, then the upper 16 bits via cubiomes biomes, then verification against the heightmap via xpple terrain gen or the existing vanilla-server regen. It needs no GPU.
- mc-locate's code (if MIT holds) could be read for its LattiCG port and cubiomes build setup. Its maturity (212 tests claimed, one week of commits) is unverified.

### Gaps
- I did not find the "kaptainwutax/seed-cracker-cli" or "Andrew's structure seed from screenshot" tools named in the brief, and cannot confirm they exist.
- I found no Dynmap- or BlueMap-specific seed crackers.

## Legal and licensing: porting Mojang code vs clean-room reimplementation

### Takeaway
Since 26.1, Java Edition ships unobfuscated, with a license file linking to the EULA. Decompiled code still belongs to Mojang, and the terms allow only modified or partial redistribution as part of a larger project. Reusing MIT community reimplementations (cubiomes, LattiCG, SeedcrackerX) is the low-risk route.

### Cited Findings
- Starting with 26.1 Snapshot 1, standard releases are no longer obfuscated. JARs include a license file linking to the EULA, and obfuscation maps are no longer shipped — [minecraft.net: Removing obfuscation](https://www.minecraft.net/en-us/article/removing-obfuscation-in-java-edition), [Minecraft Wiki: Obfuscation map](https://minecraft.wiki/w/Obfuscation_map)
- Mapping license: "You may copy and use the mappings for development purposes, but you may not redistribute the mappings complete and unmodified". The Wiki adds that decompiled source may be distributed only in modified form or as part of a larger project — [Minecraft Wiki: Obfuscation map](https://minecraft.wiki/w/Obfuscation_map), [cpw on mapping data](http://cpw.github.io/MinecraftMappingData.html)
- Licenses of reusable components: cubiomes and the xpple fork are MIT, LattiCG is MIT, rusticg is Apache-2.0, SeedcrackerX is MIT per its repo page, and Nether_Bedrock_Cracker is LGPL-3.0 — sources as cited in the sections above

### Inferences
- Reading vanilla code to match algorithms is the de facto community practice (cubiomes itself does this). Translating large decompiled sections into a published crate is the riskier case. Keeping worldgen in cubiomes (MIT) and writing only the cracking logic keeps the project clean.

### Gaps
- There was no legal analysis specific to reimplementing worldgen algorithms. This is not legal advice, and EULA wording on derivative tools was not reviewed in full.

## Recommendation-oriented summary: mature vs to-be-written

### Takeaway
Mature and reusable: the xpple cubiomes fork (biomes, structures, terrain, through 26.3), LattiCG-style lattice reversal (rusticg in Rust), and SeedcrackerX's algorithm as the reference. To be written: a maintained `-sys` binding to the xpple fork, the constraint builder from BlueMap observations (per-version structure configs), the upper-16-bit biome filter, and heightmap verification.

### Cited Findings
- Upstream cubiomes stops at 1.21 WD, while xpple supports 26.3 and terrain — [biomes.h](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomes.h), [xpple/cubiomes](https://github.com/xpple/cubiomes)
- rusticg exists (Apache-2.0, Java Random only) — [crates.io](https://crates.io/api/v1/crates/rusticg)
- SeedcrackerX's two-stage pipeline has 32–40 bits of structure data as its threshold — [SeedcrackerX README](https://github.com/19MisterX98/SeedcrackerX/blob/master/README.md)
- mc-locate shows the whole stack (Rust, LattiCG port, xpple cubiomes, no Java) is feasible on Windows — [mc-locate](https://github.com/LAOUUUUU/mc-locate)

### Inferences
- Suggested stack: vendor xpple/cubiomes as a git submodule with a project `-sys` crate (cc + bindgen), and use rusticg or a small custom lattice solver. The existing vanilla-server regen stays the final verifier.
- Brute force without lattices is also viable. There are 2^48 structure seeds, and each observed structure cuts candidates by its region-bit entropy. With many observed structures, plain enumeration plus filtering on a multicore CPU finishes in hours at worst, based on the mc-locate 2^48 ≈ 1 day on 8 cores figure.

### Gaps
- There is no public benchmark of cubiomes biome-check throughput per seed on 26.x.
- Version-specific structure placement configs (spacing, separation, salt) for 26.x were not tabulated here.
