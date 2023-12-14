
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::process::exit;
use std::time::Instant;

// minus x, plus x, minus y, plus y, minus z, plus z
const DIRECTIONS: [usize; 6] = [0, 1, 2, 3, 4, 5];
// used to create a unique integer position for each cube
//   in a polycube
const DIRECTION_COSTS: [isize; 6] = [-1, 1, -100, 100, -10_000, 10_000];
// each of the 24 possible rotations of a 3d object
//  (where each value refers to one of the above directions)
const ROTATIONS: [[u8; 6]; 24] = [
	[0,1,2,3,4,5], [0,1,3,2,5,4], [0,1,4,5,3,2], [0,1,5,4,2,3],
	[1,0,2,3,5,4], [1,0,3,2,4,5], [1,0,4,5,2,3], [1,0,5,4,3,2],
	[2,3,0,1,5,4], [2,3,1,0,4,5], [2,3,4,5,0,1], [2,3,5,4,1,0],
	[3,2,0,1,4,5], [3,2,1,0,5,4], [3,2,4,5,1,0], [3,2,5,4,0,1],
	[4,5,0,1,2,3], [4,5,1,0,3,2], [4,5,2,3,1,0], [4,5,3,2,0,1],
	[5,4,0,1,3,2], [5,4,1,0,2,3], [5,4,2,3,0,1], [5,4,3,2,1,0]];

// the rust compiler won't let me compute this ROTATION_TABLE
//   using const functions (even though they're only using
//   const data as input) so instead i've pre-computed the table
//const ROTATION_TABLE: [[u8; 24]; 64] = build_rotation_table();
const ROTATION_TABLE: [[u8; 24]; 64] = [
	[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
	[1, 2, 4, 8, 2, 1, 4, 8, 2, 1, 4, 8, 1, 2, 4, 8, 16, 16, 16, 16, 32, 32, 32, 32],
	[2, 1, 8, 4, 1, 2, 8, 4, 1, 2, 8, 4, 2, 1, 8, 4, 32, 32, 32, 32, 16, 16, 16, 16],
	[3, 3, 12, 12, 3, 3, 12, 12, 3, 3, 12, 12, 3, 3, 12, 12, 48, 48, 48, 48, 48, 48, 48, 48],
	[4, 8, 2, 1, 4, 8, 1, 2, 16, 16, 16, 16, 32, 32, 32, 32, 1, 2, 4, 8, 2, 1, 4, 8],
	[5, 10, 6, 9, 6, 9, 5, 10, 18, 17, 20, 24, 33, 34, 36, 40, 17, 18, 20, 24, 34, 33, 36, 40],
	[6, 9, 10, 5, 5, 10, 9, 6, 17, 18, 24, 20, 34, 33, 40, 36, 33, 34, 36, 40, 18, 17, 20, 24],
	[7, 11, 14, 13, 7, 11, 13, 14, 19, 19, 28, 28, 35, 35, 44, 44, 49, 50, 52, 56, 50, 49, 52, 56],
	[8, 4, 1, 2, 8, 4, 2, 1, 32, 32, 32, 32, 16, 16, 16, 16, 2, 1, 8, 4, 1, 2, 8, 4],
	[9, 6, 5, 10, 10, 5, 6, 9, 34, 33, 36, 40, 17, 18, 20, 24, 18, 17, 24, 20, 33, 34, 40, 36],
	[10, 5, 9, 6, 9, 6, 10, 5, 33, 34, 40, 36, 18, 17, 24, 20, 34, 33, 40, 36, 17, 18, 24, 20],
	[11, 7, 13, 14, 11, 7, 14, 13, 35, 35, 44, 44, 19, 19, 28, 28, 50, 49, 56, 52, 49, 50, 56, 52],
	[12, 12, 3, 3, 12, 12, 3, 3, 48, 48, 48, 48, 48, 48, 48, 48, 3, 3, 12, 12, 3, 3, 12, 12],
	[13, 14, 7, 11, 14, 13, 7, 11, 50, 49, 52, 56, 49, 50, 52, 56, 19, 19, 28, 28, 35, 35, 44, 44],
	[14, 13, 11, 7, 13, 14, 11, 7, 49, 50, 56, 52, 50, 49, 56, 52, 35, 35, 44, 44, 19, 19, 28, 28],
	[15, 15, 15, 15, 15, 15, 15, 15, 51, 51, 60, 60, 51, 51, 60, 60, 51, 51, 60, 60, 51, 51, 60, 60],
	[16, 16, 16, 16, 32, 32, 32, 32, 4, 8, 1, 2, 4, 8, 2, 1, 4, 8, 2, 1, 4, 8, 1, 2],
	[17, 18, 20, 24, 34, 33, 36, 40, 6, 9, 5, 10, 5, 10, 6, 9, 20, 24, 18, 17, 36, 40, 33, 34],
	[18, 17, 24, 20, 33, 34, 40, 36, 5, 10, 9, 6, 6, 9, 10, 5, 36, 40, 34, 33, 20, 24, 17, 18],
	[19, 19, 28, 28, 35, 35, 44, 44, 7, 11, 13, 14, 7, 11, 14, 13, 52, 56, 50, 49, 52, 56, 49, 50],
	[20, 24, 18, 17, 36, 40, 33, 34, 20, 24, 17, 18, 36, 40, 34, 33, 5, 10, 6, 9, 6, 9, 5, 10],
	[21, 26, 22, 25, 38, 41, 37, 42, 22, 25, 21, 26, 37, 42, 38, 41, 21, 26, 22, 25, 38, 41, 37, 42],
	[22, 25, 26, 21, 37, 42, 41, 38, 21, 26, 25, 22, 38, 41, 42, 37, 37, 42, 38, 41, 22, 25, 21, 26],
	[23, 27, 30, 29, 39, 43, 45, 46, 23, 27, 29, 30, 39, 43, 46, 45, 53, 58, 54, 57, 54, 57, 53, 58],
	[24, 20, 17, 18, 40, 36, 34, 33, 36, 40, 33, 34, 20, 24, 18, 17, 6, 9, 10, 5, 5, 10, 9, 6],
	[25, 22, 21, 26, 42, 37, 38, 41, 38, 41, 37, 42, 21, 26, 22, 25, 22, 25, 26, 21, 37, 42, 41, 38],
	[26, 21, 25, 22, 41, 38, 42, 37, 37, 42, 41, 38, 22, 25, 26, 21, 38, 41, 42, 37, 21, 26, 25, 22],
	[27, 23, 29, 30, 43, 39, 46, 45, 39, 43, 45, 46, 23, 27, 30, 29, 54, 57, 58, 53, 53, 58, 57, 54],
	[28, 28, 19, 19, 44, 44, 35, 35, 52, 56, 49, 50, 52, 56, 50, 49, 7, 11, 14, 13, 7, 11, 13, 14],
	[29, 30, 23, 27, 46, 45, 39, 43, 54, 57, 53, 58, 53, 58, 54, 57, 23, 27, 30, 29, 39, 43, 45, 46],
	[30, 29, 27, 23, 45, 46, 43, 39, 53, 58, 57, 54, 54, 57, 58, 53, 39, 43, 46, 45, 23, 27, 29, 30],
	[31, 31, 31, 31, 47, 47, 47, 47, 55, 59, 61, 62, 55, 59, 62, 61, 55, 59, 62, 61, 55, 59, 61, 62],
	[32, 32, 32, 32, 16, 16, 16, 16, 8, 4, 2, 1, 8, 4, 1, 2, 8, 4, 1, 2, 8, 4, 2, 1],
	[33, 34, 36, 40, 18, 17, 20, 24, 10, 5, 6, 9, 9, 6, 5, 10, 24, 20, 17, 18, 40, 36, 34, 33],
	[34, 33, 40, 36, 17, 18, 24, 20, 9, 6, 10, 5, 10, 5, 9, 6, 40, 36, 33, 34, 24, 20, 18, 17],
	[35, 35, 44, 44, 19, 19, 28, 28, 11, 7, 14, 13, 11, 7, 13, 14, 56, 52, 49, 50, 56, 52, 50, 49],
	[36, 40, 34, 33, 20, 24, 17, 18, 24, 20, 18, 17, 40, 36, 33, 34, 9, 6, 5, 10, 10, 5, 6, 9],
	[37, 42, 38, 41, 22, 25, 21, 26, 26, 21, 22, 25, 41, 38, 37, 42, 25, 22, 21, 26, 42, 37, 38, 41],
	[38, 41, 42, 37, 21, 26, 25, 22, 25, 22, 26, 21, 42, 37, 41, 38, 41, 38, 37, 42, 26, 21, 22, 25],
	[39, 43, 46, 45, 23, 27, 29, 30, 27, 23, 30, 29, 43, 39, 45, 46, 57, 54, 53, 58, 58, 53, 54, 57],
	[40, 36, 33, 34, 24, 20, 18, 17, 40, 36, 34, 33, 24, 20, 17, 18, 10, 5, 9, 6, 9, 6, 10, 5],
	[41, 38, 37, 42, 26, 21, 22, 25, 42, 37, 38, 41, 25, 22, 21, 26, 26, 21, 25, 22, 41, 38, 42, 37],
	[42, 37, 41, 38, 25, 22, 26, 21, 41, 38, 42, 37, 26, 21, 25, 22, 42, 37, 41, 38, 25, 22, 26, 21],
	[43, 39, 45, 46, 27, 23, 30, 29, 43, 39, 46, 45, 27, 23, 29, 30, 58, 53, 57, 54, 57, 54, 58, 53],
	[44, 44, 35, 35, 28, 28, 19, 19, 56, 52, 50, 49, 56, 52, 49, 50, 11, 7, 13, 14, 11, 7, 14, 13],
	[45, 46, 39, 43, 30, 29, 23, 27, 58, 53, 54, 57, 57, 54, 53, 58, 27, 23, 29, 30, 43, 39, 46, 45],
	[46, 45, 43, 39, 29, 30, 27, 23, 57, 54, 58, 53, 58, 53, 57, 54, 43, 39, 45, 46, 27, 23, 30, 29],
	[47, 47, 47, 47, 31, 31, 31, 31, 59, 55, 62, 61, 59, 55, 61, 62, 59, 55, 61, 62, 59, 55, 62, 61],
	[48, 48, 48, 48, 48, 48, 48, 48, 12, 12, 3, 3, 12, 12, 3, 3, 12, 12, 3, 3, 12, 12, 3, 3],
	[49, 50, 52, 56, 50, 49, 52, 56, 14, 13, 7, 11, 13, 14, 7, 11, 28, 28, 19, 19, 44, 44, 35, 35],
	[50, 49, 56, 52, 49, 50, 56, 52, 13, 14, 11, 7, 14, 13, 11, 7, 44, 44, 35, 35, 28, 28, 19, 19],
	[51, 51, 60, 60, 51, 51, 60, 60, 15, 15, 15, 15, 15, 15, 15, 15, 60, 60, 51, 51, 60, 60, 51, 51],
	[52, 56, 50, 49, 52, 56, 49, 50, 28, 28, 19, 19, 44, 44, 35, 35, 13, 14, 7, 11, 14, 13, 7, 11],
	[53, 58, 54, 57, 54, 57, 53, 58, 30, 29, 23, 27, 45, 46, 39, 43, 29, 30, 23, 27, 46, 45, 39, 43],
	[54, 57, 58, 53, 53, 58, 57, 54, 29, 30, 27, 23, 46, 45, 43, 39, 45, 46, 39, 43, 30, 29, 23, 27],
	[55, 59, 62, 61, 55, 59, 61, 62, 31, 31, 31, 31, 47, 47, 47, 47, 61, 62, 55, 59, 62, 61, 55, 59],
	[56, 52, 49, 50, 56, 52, 50, 49, 44, 44, 35, 35, 28, 28, 19, 19, 14, 13, 11, 7, 13, 14, 11, 7],
	[57, 54, 53, 58, 58, 53, 54, 57, 46, 45, 39, 43, 29, 30, 23, 27, 30, 29, 27, 23, 45, 46, 43, 39],
	[58, 53, 57, 54, 57, 54, 58, 53, 45, 46, 43, 39, 30, 29, 27, 23, 46, 45, 43, 39, 29, 30, 27, 23],
	[59, 55, 61, 62, 59, 55, 62, 61, 47, 47, 47, 47, 31, 31, 31, 31, 62, 61, 59, 55, 61, 62, 59, 55],
	[60, 60, 51, 51, 60, 60, 51, 51, 60, 60, 51, 51, 60, 60, 51, 51, 15, 15, 15, 15, 15, 15, 15, 15],
	[61, 62, 55, 59, 62, 61, 55, 59, 62, 61, 55, 59, 61, 62, 55, 59, 31, 31, 31, 31, 47, 47, 47, 47],
	[62, 61, 59, 55, 61, 62, 59, 55, 61, 62, 59, 55, 62, 61, 59, 55, 47, 47, 47, 47, 31, 31, 31, 31],
	[63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63]];

const MAXIMUM_ROTATED_CUBE_VALUES: [u8; 64] = [
	 0, 32, 32, 48, 32, 40, 40, 56,
	32, 40, 40, 56, 48, 56, 56, 60,
	32, 40, 40, 56, 40, 42, 42, 58,
	40, 42, 42, 58, 56, 58, 58, 62,
	32, 40, 40, 56, 40, 42, 42, 58,
	40, 42, 42, 58, 56, 58, 58, 62,
	48, 56, 56, 60, 56, 58, 58, 62,
	56, 58, 58, 62, 60, 62, 62, 63];

const MAXIMUM_CUBE_ROTATION_INDICES: [&[u8]; 64] = [
	&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23],
	&[20, 21, 22, 23],
	&[16, 17, 18, 19],
	&[16, 17, 18, 19, 20, 21, 22, 23],
	&[12, 13, 14, 15],
	&[15, 23],
	&[14, 19],
	&[19, 23],
	&[8, 9, 10, 11],
	&[11, 22],
	&[10, 18],
	&[18, 22],
	&[8, 9, 10, 11, 12, 13, 14, 15],
	&[11, 15],
	&[10, 14],
	&[10, 11, 14, 15, 18, 19, 22, 23],
	&[4, 5, 6, 7],
	&[7, 21],
	&[6, 17],
	&[17, 21],
	&[5, 13],
	&[7, 13, 23],
	&[5, 14, 17],
	&[17, 23],
	&[4, 9],
	&[4, 11, 21],
	&[6, 9, 18],
	&[18, 21],
	&[9, 13],
	&[11, 13],
	&[9, 14],
	&[11, 14, 18, 23],
	&[0, 1, 2, 3],
	&[3, 20],
	&[2, 16],
	&[16, 20],
	&[1, 12],
	&[1, 15, 20],
	&[2, 12, 19],
	&[19, 20],
	&[0, 8],
	&[3, 8, 22],
	&[0, 10, 16],
	&[16, 22],
	&[8, 12],
	&[8, 15],
	&[10, 12],
	&[10, 15, 19, 22],
	&[0, 1, 2, 3, 4, 5, 6, 7],
	&[3, 7],
	&[2, 6],
	&[2, 3, 6, 7, 16, 17, 20, 21],
	&[1, 5],
	&[1, 7],
	&[2, 5],
	&[2, 7, 17, 20],
	&[0, 4],
	&[3, 4],
	&[0, 6],
	&[3, 6, 16, 21],
	&[0, 1, 4, 5, 8, 9, 12, 13],
	&[1, 4, 8, 13],
	&[0, 5, 9, 12],
	&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]];

// from https://oeis.org/A000162
// these are the number of unique polycubes of size n,
//   which is kind of funny to put in a program that
//   calculates these values -- but these are needed to
//   help calculate estimated time remaining
//const WELL_KNOWN_N_COUNTS: [usize; 17] = [0, 1, 1, 2, 8, 29, 166, 1023, 6922, 48311, 346543, 2522522, 18598427, 138462649, 1039496297, 7859514470, 59795121480];

// 5 -> [false, false, false, 1, false, 1]
/*
fn int_to_bit_list(n: u8) -> [bool; 6] {
	let mut bit_list: [bool; 6] = [false; 6];
	for i in 0..6 {
		//bit_list[i] = ((n >> i) & 1) == 1;
		//bit_list[i] = n & (1 << (5-i)) == 1;
		bit_list[5-i] = (n >> i) & 1 == 1;
	}
	return bit_list;
}

fn bit_list_to_str(bit_list: [bool; 6]) -> String {
	let mut s: String = String::new();
	for i in 0..6 {
		if bit_list[i] {
			s.push('1');
		} else {
			s.push('0');
		}
	}
	return s;
}

fn int_to_bit_str(n: u8) -> String {
	return bit_list_to_str(int_to_bit_list(n));
}

fn neighbors_to_str(neighbors: [Option<isize>; 6]) -> String {
	let mut s: String = String::new();
	s.push_str("[");
	for i in 0..6 {
		match neighbors[i] {
			Some(n) => {
				s.push_str(&format!("{}, ", n));
			}
			None => {
				s.push_str("None, ");
			}
		}
	}
	s.push_str("]");
	return s;
}
*/

/*
// apply rotation: grab the ith bit from the list
fn rotate_bit_list(bit_list: [bool; 6], rotation: [u8; 6]) -> [bool; 6] {
	let mut new_bit_list: [bool; 6] = [false; 6];
	for i in 0..6 {
		new_bit_list[i] = bit_list[rotation[i] as usize];
	}
	return new_bit_list;
}

// [0, 0, 0, 1, 0, 1] -> 5
fn bit_list_to_int(bit_list: [bool; 6]) -> u8 {
	let mut n: u8 = 0;
	for i in 0..6 {
		if bit_list[i] {
			n |= 1 << (5-i);
		}
	}
	return n;
}

fn rotate_value(cube_enc: u8, rotation: [u8; 6]) -> u8 {
	return bit_list_to_int(rotate_bit_list(int_to_bit_list(cube_enc), rotation))
}

fn build_rotation_table() -> [[u8; 24]; 64] {
	let mut table: [[u8; 24]; 64] = [[0; 24]; 64];
	// for each possible grouping of presence (1) or absence (0) of a cube's 6 neighbors (2^6=64 possibilities)
	for cube_enc in 0..64 {
		// apply each of the 24 possible rotations of a 3d object
		for i in 0..24 {
			table[cube_enc as usize][i] = rotate_value(cube_enc, ROTATIONS[i])
		}
	}
	return table;
}
*/

pub struct CanonicalInfo {
	// at 6 bits per cube, 128 bits is enough room for a polycube of
	//   size 21, but polycubes have never been enumerated past n=18
	//   so 128 bits is plenty long for now
	enc: u128,
	least_significant_cube_pos: BTreeSet<isize>,
	//max_cube_values: BTreeSet<u8>
	max_cube_value: u8
}

impl CanonicalInfo {
	pub fn clone(&self) -> CanonicalInfo {
		CanonicalInfo {
			enc: self.enc,
			least_significant_cube_pos: self.least_significant_cube_pos.clone(),
			max_cube_value: self.max_cube_value
		}
	}
}

pub struct Polycube {
	// number of cubes in this polycube
	n: u8,
	canonical_info: Option<CanonicalInfo>,
	// keys - positions of cubes in this polycube
	// vals - neighbor encoding for the cube at that position
	enc_by_cube: HashMap<isize, u8>,
	// keys - positions of cubes in this polycube
	// vals - neighbors of the cube at that position in DIRECTIONS order
	neighbors_by_cube: HashMap<isize, [Option<isize>; 6]>
}

impl Polycube {
	pub fn new(create_initial_cube: bool) -> Polycube {
		// initialize with 1 cube at (0, 0, 0)
		if create_initial_cube {
			Polycube {
				n: 1,
				canonical_info: None,
				enc_by_cube: HashMap::from([(0, 0)]),
				neighbors_by_cube: HashMap::from([(0, [None, None, None, None, None, None])])
			}
		// intialize with no cubes
		} else {
			Polycube {
				n: 0,
				canonical_info: None,
				enc_by_cube: HashMap::new(),
				neighbors_by_cube: HashMap::new()
			}
		}
	}

	pub fn copy(&self) -> Polycube {
		match self.canonical_info {
			Some(ref canonical_info) => {
				Polycube {
					n: self.n,
					canonical_info: Some(CanonicalInfo {
						enc: canonical_info.enc,
						least_significant_cube_pos: canonical_info.least_significant_cube_pos.clone(),
						//max_cube_values: canonical_info.max_cube_values.clone()
						max_cube_value: canonical_info.max_cube_value
					}),
					enc_by_cube: self.enc_by_cube.clone(),
					neighbors_by_cube: self.neighbors_by_cube.clone()
				}
			}
			None => {
				Polycube {
					n: self.n,
					canonical_info: None,
					enc_by_cube: self.enc_by_cube.clone(),
					neighbors_by_cube: self.neighbors_by_cube.clone()
				}
			}
		}
	}

	pub fn add(&mut self, pos: isize) {
//		print!("adding cube at pos: [{}] for polycube of n={}\n", pos, self.n);
		let mut new_enc: u8 = 0;
		let mut new_neighbors: [Option<isize>; 6] = [None, None, None, None, None, None];

		// update each of our cube's enc values for the default
		//   rotation of [0,1,2,3,4,5]
		// set the neighbors for the new cube and set it as a neighbor to those cubes
		for direction in DIRECTIONS.iter() {
			// neighbor cube position in the direction
			let neighbor_pos = pos + DIRECTION_COSTS[*direction];
			// if there is no neightbor cube in this direction, continue to next direction
			match self.enc_by_cube.get(&neighbor_pos) {
				Some(neighbor_enc) => {
//					print!("    neighbor in dir [{}] at pos [{}] has old enc [{}]->[{}]\n", direction, neighbor_pos, neighbor_enc, int_to_bit_str(*neighbor_enc));
					new_neighbors[*direction] = Some(neighbor_pos);
					// we use rotation of [0,1,2,3,4,5] where the '0'
					//   direction is -x and is the most significant bit
					//   in each cube's .enc value, so we need '0' to
					//   cause a left shift by 5 bits
					new_enc |= 1 << (5-direction);
					// use XOR to flip between each direction and its opposite
					//   to set the neighbor's neighbor to the added cube
					//   (0<->1, 2<->3, 4<->5)
					self.neighbors_by_cube.get_mut(&neighbor_pos).unwrap()[direction ^ 1] = Some(pos);
					// we use rotation of [0,1,2,3,4,5] where the '0'
					//   direction is -x and is the most significant bit
					//   in each cube's .enc value, so we need '0' to
					//   cause a left shift by 5 bits (and here we use
					//   XOR to flip to the opposite direction)
//					let tmp_debug_e = neighbor_enc | (1 << ((5-direction) ^ 1));
//					let tmp_debug_n = self.neighbors_by_cube.get(&neighbor_pos).unwrap();
					self.enc_by_cube.insert(neighbor_pos, neighbor_enc | (1 << ((5-direction) ^ 1)));
//					print!("    neighbor in dir [{}] at pos [{}] has enc [{}]->[{}]\n", direction, neighbor_pos, tmp_debug_e, int_to_bit_str(tmp_debug_e));
//					print!("    neighbor in dir [{}] at neighbor_pos [{}] has neighbors [{}]\n", direction, neighbor_pos, neighbors_to_str(*tmp_debug_n));
				}
				None => {}
			}
		}
		// lastly, insert the new cube's encoded neighbors into our map
//		print!("    new cube at pos [{}] has enc [{}]->[{}]\n", pos, new_enc, int_to_bit_str(new_enc));
//		print!("    new cube at pos [{}] has neighbors [{}]\n", pos, neighbors_to_str(new_neighbors));
		self.enc_by_cube.insert(pos, new_enc);
		self.neighbors_by_cube.insert(pos, new_neighbors);
		self.n += 1;
		self.canonical_info = None;
	}

	pub fn remove(&mut self, pos: isize) {
//		print!("removing cube at pos: {}\n", pos);
		// remove this cube from each of its neighbors
		// create a Vec since we can't modify an HashMap while iterating over it
		let mut neighbor_dirs_to_remove_from: Vec<(isize, usize)> = Vec::with_capacity(6);
		for (dir, neighbor_pos_opt) in self.neighbors_by_cube[&pos].iter().enumerate() {
			match neighbor_pos_opt {
				Some(neighbor_pos) => {
					let neighbor_enc_orig = self.enc_by_cube.get(&neighbor_pos).unwrap();
//					print!("    neighbor at pos [{}] has enc [{}]->[{}]\n", neighbor_pos, neighbor_enc_orig, int_to_bit_str(*neighbor_enc_orig));
					// we use rotation of [0,1,2,3,4,5] where the '0'
					//   direction is -x and is the most significant bit
					//   in each cube's .enc value, so we need '0' to
					//   cause a left shift by 5 bits (then here we take
					//   the mirror with XOR)
//					print!("    looking at dir [{}]\n", dir);
//					print!("    subtracting [{}]->[{}]\n", 1 << ((5-dir) ^ 1), int_to_bit_str(1 << ((5-dir) ^ 1)));
					self.enc_by_cube.insert(*neighbor_pos, *neighbor_enc_orig - (1 << ((5-dir) ^ 1)));
					neighbor_dirs_to_remove_from.push((*neighbor_pos, dir ^ 1));
					// someday perhaps faster to use .push_within_capacity()
					//neighbor_dirs_to_remove_from.push_within_capacity((*neighbor_pos, dir ^ 1));
				}
				None => {}
			}
		}
		for (neighbor_pos, dir) in neighbor_dirs_to_remove_from {
			self.neighbors_by_cube.get_mut(&neighbor_pos).unwrap()[dir] = None;
		}
		self.enc_by_cube.remove(&pos);
		self.neighbors_by_cube.remove(&pos);
		self.n -= 1;
		self.canonical_info = None;
	}

	// for each cube, find its maximum value after a would-be rotation,
	//   and return the sorted list of those values
	// this lets us find which cube to start our maximal encoding with
	//   (and which rotation to use for that).
	// TODO: can we just use .max() instead of .sort() ??
	pub fn find_maximum_cube_values(&self) -> Vec<u8> {
		// .collect() can't infer type somehow?  sorry but that's dumb
		//let max_vals: Vec<u8> = self.enc_by_cube.values().map(|enc: &u8| MAXIMUM_ROTATED_CUBE_VALUES[*enc as usize]).collect().sort();
		//return max_vals;
		let mut max_vals: Vec<u8> = Vec::new();
		for cube_enc in self.enc_by_cube.values() {
			max_vals.push(MAXIMUM_ROTATED_CUBE_VALUES[*cube_enc as usize]);
		}
		max_vals.sort();
		return max_vals;
	}

	// since the maximal "canonical" encoding of our polycube must start with
	//   a start_cube+rotation that results in the largest possible cube_enc
	//   value, we only need to find what that single largest value is
	// (so we'll use this function instead of the above find_maximum_cube_values())
	pub fn find_maximum_cube_value(&self) -> u8 {
		return self.enc_by_cube.values().map(|enc: &u8| MAXIMUM_ROTATED_CUBE_VALUES[*enc as usize]).max().unwrap();
	}

	pub fn make_encoding_recursive(
			&self,
			start_cube_pos: isize,
			rotation: [u8; 6],
			included_cube_pos: &mut HashSet<isize>,
			best_encoding: u128,
			rotations_index: usize,
			mut offset: u8,
			mut encoding: u128) -> Option<(Vec<isize>, u128, u8)> {
		encoding = (encoding << 6) + (ROTATION_TABLE[self.enc_by_cube[&start_cube_pos] as usize][rotations_index] as u128);
		// as soon as we can tell this is going to be an inferior encoding
		//   (smaller int value than the given best known encofing)
		//   we can stop right away
		if encoding < (best_encoding >> (offset * 6)) {
			return None;
		}
		let mut ordered_cubes: Vec<isize> = Vec::from([start_cube_pos]);
		included_cube_pos.insert(start_cube_pos);
		for direction in rotation {
			match self.neighbors_by_cube[&start_cube_pos][direction as usize] {
				Some(neighbor_pos) => {
					if included_cube_pos.contains(&neighbor_pos) {
						continue;
					}
					match self.make_encoding_recursive(
							neighbor_pos,
							rotation,
							included_cube_pos,
							best_encoding,
							rotations_index,
							offset - 1,
							encoding) {
						Some((mut ordered_cubes_new, encoding_ret, offset_ret)) => {
							ordered_cubes.append(&mut ordered_cubes_new);
							encoding = encoding_ret;
							offset = offset_ret;
						}
						// if the Option is empty, that means we have determined
						//   somewhere deeper in the recursion that this is
						//   a dead-end inferior encoding, so we can stop
						None => {
							return None
						}
					}
				}
				// if there is no neighbor in this direction just continue
				None => {}
			}
		}
		return Some((ordered_cubes, encoding, offset));
	}

	pub fn make_encoding(&self, start_cube_pos: isize, rotations_index: usize, best_encoding: u128) -> Option<(u128, isize)> {
		let mut included_cube_pos: HashSet<isize> = HashSet::new();
		// uses a recursive depth-first encoding of all cubes, using
		//   the provided rotation's order to traverse the cubes
		match self.make_encoding_recursive(
				start_cube_pos,
				ROTATIONS[rotations_index],
				&mut included_cube_pos,
				best_encoding,
				rotations_index,
				self.n - 1, // number of 6-bit shifts from the right, where the last cube has an offset of 0
				0) {
			Some((ordered_cubes, encoding, _offset)) => {
				return Some((encoding, *ordered_cubes.last().unwrap()));
			}
			// if the Option is empty, that means we have determined 
			//   somewhere deeper in the recursion that this is
			//   a dead-end inferior encoding, so we can stop
			None => {
				return None;
			}
		}
	}

	// return our canonical info, calculating it first if necessary
	pub fn find_canonical_info(&mut self) -> &CanonicalInfo {
		if self.canonical_info.is_none() {
			let mut canonical = CanonicalInfo {
				enc: 0,
				least_significant_cube_pos: BTreeSet::new(),
				max_cube_value: self.find_maximum_cube_value()
			};
			let mut best_encoding: u128 = 0;
			let mut encoding_diff: u128;
			for (cube_pos, cube_enc) in self.enc_by_cube.iter() {
				// there could be more than one cube with the maximum rotated value
				if MAXIMUM_ROTATED_CUBE_VALUES[*cube_enc as usize] < canonical.max_cube_value {
					continue;
				}
				for rotations_index in MAXIMUM_CUBE_ROTATION_INDICES[*cube_enc as usize].iter() {
					match self.make_encoding(*cube_pos, *rotations_index as usize, best_encoding) {
						Some((encoding, least_significant_cube_pos)) => {
							encoding_diff = encoding - best_encoding;
							if encoding_diff > 0 {
								canonical.enc = encoding;
								canonical.least_significant_cube_pos.clear();
								canonical.least_significant_cube_pos.insert(least_significant_cube_pos);
								best_encoding = encoding;
							} else if encoding_diff == 0 {
								canonical.least_significant_cube_pos.insert(least_significant_cube_pos);
							}
						}
						// if the Option is empty, that means we have determined 
						//   somewhere in the recursion that this is a dead-end
						//   inferior encoding, so we can try the next rotation
						None => {
							continue;
						}
					}
				}
			}
			self.canonical_info = Some(canonical);
		}
		return self.canonical_info.as_ref().unwrap();
	}
}

static mut N_COUNTS: [usize; 23] = [0; 23];

pub fn extend_single_thread(polycube: &mut Polycube, limit_n: u8, depth: usize) {
	// since this is a valid polycube, increment the count
	unsafe {
		N_COUNTS[polycube.n as usize] += 1;
	}

	// we are done if we've reached the desired n,
	//   which we need to stop at because we are doing
	//   a depth-first recursive evaluation
	if polycube.n == limit_n {
		return;
	}

	// keep a Set of all evaluated positions so we don't repeat them
	let mut tried_pos: HashSet<isize> = HashSet::new();

	let mut tried_canonicals: HashSet<u128> = HashSet::new();

	// i'd like to not clone this, but that might not be possible
	let canonical_orig: CanonicalInfo = polycube.find_canonical_info().clone();
	let mut canonical_try: &CanonicalInfo;
	let mut least_significant_cube_pos: isize;

	let mut try_pos: isize;

	// for each cube, for each direction, add a cube
	// create a list to iterate over because the dict will change
	//   during recursion within the loop
	let original_positions: Vec<isize> = polycube.enc_by_cube.keys().cloned().collect();
	// include all existing cubes' positions in the tried_pos set
	tried_pos.extend(original_positions.iter());
	for cube_pos in original_positions {
		for direction_cost in DIRECTION_COSTS {
			try_pos = cube_pos + direction_cost;
			// skip if we've already tried this position
			if !tried_pos.insert(try_pos) {
//				print!("{}skipping already tried try_pos=[{}]\n", " ".repeat(depth*4), try_pos);
				continue;
			}

			// create p+1
//			print!("{}adding try_pos=[{}]\n", " ".repeat(depth*4), try_pos);
			polycube.add(try_pos);

			// skip if we've already seen some p+1 with the same canonical representation
			//   (comparing the bitwise int only)
			canonical_try = polycube.find_canonical_info();
			if !tried_canonicals.insert(canonical_try.enc) {
//				print!("{}removing try_pos=[{}]\n", " ".repeat(depth*4), try_pos);
				polycube.remove(try_pos);
				continue;
			}

			// remove the last of the ordered cubes in p+1
			least_significant_cube_pos = canonical_try.least_significant_cube_pos.first().unwrap().clone();

//			print!("{}removing least sig=[{}]\n", " ".repeat(depth*4), least_significant_cube_pos);
			polycube.remove(least_significant_cube_pos);

			// if p+1-1 has the same canonical representation as p, count it as a new unique polycube
			//   and continue recursion into that p+1
			if polycube.find_canonical_info().enc == canonical_orig.enc {
				// replace the least significant cube we just removed
//				print!("{}replacing least sig=[{}]\n", " ".repeat(depth*4), least_significant_cube_pos);
				polycube.add(least_significant_cube_pos);
				extend_single_thread(polycube, limit_n, depth+1);
			
			// undo the temporary removal of the least significant cube,
			//   but only if it's not the same as the cube we just tried
			//   since we remove that one before going to the next iteration
			//   of the loop
			} else if least_significant_cube_pos != try_pos {
//				print!("{}replacing least sig=[{}]\n", " ".repeat(depth*4), least_significant_cube_pos);
				polycube.add(least_significant_cube_pos);
			}

			// revert creating p+1 to try adding a cube at another position
//			print!("{}removing try_pos=[{}]\n", " ".repeat(depth*4), try_pos);
			polycube.remove(try_pos);
		}
	}
}

pub fn validate_resume_file(resume_file: &str) -> Result<PathBuf, String> {
    let resume_file_path = PathBuf::from(resume_file);
    if !resume_file_path.is_file() {
        return Err(format!("<resume-file> [{}] is not a regular file, does not exist, or does not have permissions \
            necessary for access", resume_file));
    }
    Ok(resume_file_path)
}

pub fn print_results(n: u8) {
	unsafe {
		println!("\n\nresults:");
		for i in 1..n+1 {
			println!("n = {: >2}: {}", i, N_COUNTS[i as usize]);
		}
	}
}

fn main() {
	let args: Vec<String> = env::args().collect();
	let usage = format!("usage: {} [--n <n>] [--threads <threads>] [--spawn-n <spawn-n>] [--resume-from-file <resume-file>]\n\
	where:\n\
	-  <n>          : the number of cubes the largest counted polycube should contain (>1)\n\
	-  <threads>    : 0 for single-threaded, or >1 for the maximum number of threads to spawn simultaneously (default=0)\n\
	-  <spawn-n>    : the smallest polycubes for which each will spawn a thread, higher->more shorter-lived threads (default=8)\n\
	-  <resume-file>: a .json.gz file previously created by this program",
	args[0]);
	if args.len() < 3 {
		println!("{}", usage);
		exit(1);
	}

	let mut cursor: usize = 1;
	let mut arg_n: u8 = 0;
	let mut arg_threads: u8 = 0;
	let mut arg_spawn_n: u8 = 8;
	let mut arg_resume_file: Option<PathBuf> = None;
	// we want to start at the 1th index, and advance by 2
	while cursor < args.len() - 1 {
		if args[cursor] == "--help" || args[cursor] == "-h" {
			println!("{}", usage);
			exit(1);
		}
		if args[cursor] == "--n" || args[cursor] == "-n" {
			arg_n = match args[cursor + 1].parse() {
				Ok(n) => {
					if n < 2 {
						println!("error: n must be greater than 1");
						println!("{}", usage);
						exit(1);
					}
					n
				}
				Err(_) => {
					println!("error: invalid value for n");
					println!("{}", usage);
					exit(1);
				}
			};
		}
		if args[cursor] == "--threads" || args[cursor] == "-t" {
			arg_threads = match args[cursor + 1].parse() {
				Ok(threads) => {
					if threads == 1 {
						println!("error: threads must be 0 or greater than 1");
						println!("{}", usage);
						exit(1);
					}
					threads
				}
				Err(_) => {
					println!("error: invalid value for threads");
					println!("{}", usage);
					exit(1);
				}
			};
		}
		if args[cursor] == "--spawn-n" || args[cursor] == "-s" {
			arg_spawn_n = match args[cursor + 1].parse() {
				Ok(spawn_n) => {
					if spawn_n < 4 {
						println!("error: spawn-n must be greater than 3");
						println!("{}", usage);
						exit(1);
					}
					spawn_n
				}
				Err(_) => {
					println!("error: invalid value for spawn-n");
					println!("{}", usage);
					exit(1);
				}
			};
		}
		if args[cursor] == "--resume-from-file" || args[cursor] == "-r" {
			arg_resume_file = match validate_resume_file(&args[cursor + 1]) {
				Ok(path) => Some(path),
				Err(err) => {
					println!("error: {}", err);
					println!("{}", usage);
					exit(1);
				}
			};
		}
		cursor += 2;
	}
	// we either need a <resume-file> or a value for <n>
	match arg_resume_file {
		Some(resume_file_path) => {
			println!("resuming from file: {}", resume_file_path.to_str().unwrap());
		}
		None => {
			if arg_n == 0 {
				println!("error: n must be specified");
				println!("{}", usage);
				exit(1);
			}
		}
	}
	print!("spawn n: {}\n", arg_spawn_n); // just print this to avoid unused warning
	let start_time = Instant::now();
	let mut last_count_increment_time: Option<Instant> = None;
	let mut polycube: Polycube = Polycube::new(true);
	if arg_threads == 0 {
		extend_single_thread(&mut polycube, arg_n, 0);
	} else {
		println!("multi-thread not implemented yet");
	}
	if last_count_increment_time.is_none() {
		last_count_increment_time = Some(Instant::now());
	}
	print_results(arg_n);
	let total_duration = last_count_increment_time.unwrap().duration_since(start_time);
	println!("elapsed seconds: {}.{}", total_duration.as_secs(), total_duration.subsec_micros());
}