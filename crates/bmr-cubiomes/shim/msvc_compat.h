// cubiomes is GCC-oriented; with this and the `bmr patch` edits in vendor/ it builds with MSVC
#pragma once
#include <intrin.h>
#include <malloc.h>
#define __builtin_popcountll(x) ((int)__popcnt64(x))
#define alloca _alloca
