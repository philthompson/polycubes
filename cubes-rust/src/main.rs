
use chrono::prelude::*;
use crossbeam_queue::ArrayQueue;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use rand::prelude::*;
use std::collections::BTreeSet;
use std::collections::BTreeMap;
use std::env;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::thread::JoinHandle;
//use std::thread::Thread;
use std::time::Duration;
use std::time::Instant;

// minus x, plus x, minus y, plus y, minus z, plus z
const DIRECTIONS: [usize; 6] = [0, 1, 2, 3, 4, 5];
// used to create a unique integer position for each cube
//   in a polycube
const DIRECTION_COSTS: [isize; 6] = [-1, 1, -100, 100, -10_000, 10_000];
// each of the 24 possible rotations of a 3d object
//  (where each value refers to one of the above directions)
const ROTATIONS: [[usize; 6]; 24] = [
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
const WELL_KNOWN_N_COUNTS: [usize; 17] = [0, 1, 1, 2, 8, 29, 166, 1023, 6922, 48311, 346543, 2522522, 18598427, 138462649, 1039496297, 7859514470, 59795121480];

// store counts for 0 cubes, 1 cube, 2 cubes, etc, up to MAX_N-1
const MAX_N: usize = 22+1;

// since we offset in the +x direction by 1 unit for each cube,
//   and enumerating cubes for n=50 is way beyond what's possible,
//   we can use pos=50 as a placeholder for an "impossible" pos
const IMPOSSIBLE_POS: isize = 50;

pub struct ThreadResponse {
	pub job_complete: bool,
	pub results: Option<[usize; MAX_N]>,
	pub polycube: Option<Polycube>
}
pub struct CanonicalInfo {
	// at 6 bits per cube, 128 bits is enough room for a polycube of
	//   size 21, but polycubes have never been enumerated past n=18
	//   so 128 bits is plenty long for now
	enc: u128,
	least_significant_cube_pos: isize,
	max_cube_value: u8
}

impl CanonicalInfo {
	pub fn clone(&self) -> CanonicalInfo {
		CanonicalInfo {
			enc: self.enc,
			least_significant_cube_pos: self.least_significant_cube_pos,
			max_cube_value: self.max_cube_value
		}
	}
}

pub struct Polycube {
	// number of cubes in this polycube
	n: u8,
	canonical_info: Option<CanonicalInfo>,
	// keys     - positions of cubes in this polycube
	// vals 0-5 - neighbors of the cube at that position in DIRECTIONS order
	// val  6   - neighbor encoding for the cube at that position
	cube_info_by_pos: BTreeMap<isize, [Option<isize>; 7]>
}

impl Polycube {
	pub fn new(create_initial_cube: bool) -> Polycube {
		// initialize with 1 cube at (0, 0, 0)
		if create_initial_cube {
			Polycube {
				n: 1,
				canonical_info: None,
				cube_info_by_pos: BTreeMap::from([(0, [None, None, None, None, None, None, Some(0)])])
			}
		// intialize with no cubes
		} else {
			Polycube {
				n: 0,
				canonical_info: None,
				cube_info_by_pos: BTreeMap::new()
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
						max_cube_value: canonical_info.max_cube_value
					}),
					cube_info_by_pos: self.cube_info_by_pos.clone()
				}
			}
			None => {
				Polycube {
					n: self.n,
					canonical_info: None,
					cube_info_by_pos: self.cube_info_by_pos.clone()
				}
			}
		}
	}

	// same as add_loop() below but with the loop unrolled
	pub fn add(&mut self, pos: isize) {
		let mut new_enc: isize = 0;
		let mut new_info: [Option<isize>; 7] = [None, None, None, None, None, None, Some(0)];

		// update each of our cube's enc values for the default
		//   rotation of [0,1,2,3,4,5]
		// set the neighbors for the new cube and set it as a neighbor to those cubes

		// direction = 0 -> direction cost = -1
		let mut neighbor_pos = pos - 1;
		match self.cube_info_by_pos.get_mut(&neighbor_pos) {
			Some(neighbor_info) => {
				new_info[0] = Some(neighbor_pos);
				new_enc |= 32;
				neighbor_info[1] = Some(pos);
				neighbor_info[6] = Some(neighbor_info[6].unwrap() | 16);
			}
			None => {}
		}
		// direction = 1 -> direction cost = 1
		neighbor_pos = pos + 1;
		match self.cube_info_by_pos.get_mut(&neighbor_pos) {
			Some(neighbor_info) => {
				new_info[1] = Some(neighbor_pos);
				new_enc |= 16;
				neighbor_info[0] = Some(pos);
				neighbor_info[6] = Some(neighbor_info[6].unwrap() | 32);
			}
			None => {}
		}
		// direction = 2 -> direction cost = -100
		neighbor_pos = pos - 100;
		match self.cube_info_by_pos.get_mut(&neighbor_pos) {
			Some(neighbor_info) => {
				new_info[2] = Some(neighbor_pos);
				new_enc |= 8;
				neighbor_info[3] = Some(pos);
				neighbor_info[6] = Some(neighbor_info[6].unwrap() | 4);
			}
			None => {}
		}
		// direction = 3 -> direction cost = 100
		neighbor_pos = pos + 100;
		match self.cube_info_by_pos.get_mut(&neighbor_pos) {
			Some(neighbor_info) => {
				new_info[3] = Some(neighbor_pos);
				new_enc |= 4;
				neighbor_info[2] = Some(pos);
				neighbor_info[6] = Some(neighbor_info[6].unwrap() | 8);
			}
			None => {}
		}
		// direction = 4 -> direction cost = -10000
		neighbor_pos = pos - 10000;
		match self.cube_info_by_pos.get_mut(&neighbor_pos) {
			Some(neighbor_info) => {
				new_info[4] = Some(neighbor_pos);
				new_enc |= 2;
				neighbor_info[5] = Some(pos);
				neighbor_info[6] = Some(neighbor_info[6].unwrap() | 1);
			}
			None => {}
		}
		// direction = 5 -> direction cost = 10000
		neighbor_pos = pos + 10000;
		match self.cube_info_by_pos.get_mut(&neighbor_pos) {
			Some(neighbor_info) => {
				new_info[5] = Some(neighbor_pos);
				new_enc |= 1;
				neighbor_info[4] = Some(pos);
				neighbor_info[6] = Some(neighbor_info[6].unwrap() | 2);
			}
			None => {}
		}
		// lastly, insert the new cube's encoded neighbors into our map
		new_info[6] = Some(new_enc);
		self.cube_info_by_pos.insert(pos, new_info);
		self.n += 1;
		self.canonical_info = None;
	}

	// this is the original loop that was unrolled above in add()
	pub fn add_loop(&mut self, pos: isize) {
		let mut new_enc: isize = 0;
		let mut new_info: [Option<isize>; 7] = [None, None, None, None, None, None, Some(0)];

		// update each of our cube's enc values for the default
		//   rotation of [0,1,2,3,4,5]
		// set the neighbors for the new cube and set it as a neighbor to those cubes
		for direction in DIRECTIONS.iter() {
			// neighbor cube position in the direction
			let neighbor_pos = pos + DIRECTION_COSTS[*direction];
			// if there is no neightbor cube in this direction, continue to next direction
			match self.cube_info_by_pos.get_mut(&neighbor_pos) {
				Some(neighbor_info) => {
					new_info[*direction] = Some(neighbor_pos);
					// we use rotation of [0,1,2,3,4,5] where the '0'
					//   direction is -x and is the most significant bit
					//   in each cube's .enc value, so we need '0' to
					//   cause a left shift by 5 bits
					new_enc |= 1 << (5-direction);
					// use XOR to flip between each direction and its opposite
					//   to set the neighbor's neighbor to the added cube
					//   (0<->1, 2<->3, 4<->5)
					neighbor_info[direction ^ 1] = Some(pos);
					// we use rotation of [0,1,2,3,4,5] where the '0'
					//   direction is -x and is the most significant bit
					//   in each cube's .enc value, so we need '0' to
					//   cause a left shift by 5 bits (and here we use
					//   XOR to flip to the opposite direction)
					neighbor_info[6] = Some(neighbor_info[6].unwrap() | (1 << ((5-direction) ^ 1)));
				}
				None => {}
			}
		}
		// lastly, insert the new cube's encoded neighbors into our map
		new_info[6] = Some(new_enc);
		self.cube_info_by_pos.insert(pos, new_info);
		self.n += 1;
		self.canonical_info = None;
	}

	pub fn remove(&mut self, pos: isize) {
		// first remove the cube's data from our map
		let cube_info = self.cube_info_by_pos.remove(&pos).unwrap();
		// remove this cube from each of its neighbors
		for (dir, neighbor_pos) in (&cube_info[0..6]).iter().enumerate() {
			match neighbor_pos {
				Some(neighbor_pos) => {
					match self.cube_info_by_pos.get_mut(&neighbor_pos) {
						Some(neighbor_info) => {
							// we use rotation of [0,1,2,3,4,5] where the '0'
							//   direction is -x and is the most significant bit
							//   in each cube's .enc value, so we need '0' to
							//   cause a left shift by 5 bits (and here we use
							//   XOR to flip to the opposite direction)
							neighbor_info[6] = Some(neighbor_info[6].unwrap() - (1 << ((5-dir) ^ 1)));
							// use XOR to flip between each direction and its opposite
							//   to set the neighbor's neighbor to None
							//   (0<->1, 2<->3, 4<->5)
							neighbor_info[dir ^ 1] = None;
						}
						// panic here?  we should never have a cube with a neighbor
						//   whose info doesn't exist in the map
						None => {}
					}
				}
				None => {}
			}
		}
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
		for cube_info in self.cube_info_by_pos.values() {
			max_vals.push(MAXIMUM_ROTATED_CUBE_VALUES[cube_info[6].unwrap() as usize]);
		}
		max_vals.sort();
		return max_vals;
	}

	// since the maximal "canonical" encoding of our polycube must start with
	//   a start_cube+rotation that results in the largest possible cube_enc
	//   value, we only need to find what that single largest value is
	// (so we'll use this function instead of the above find_maximum_cube_values())
	pub fn find_maximum_cube_value(&self) -> u8 {
		return self.cube_info_by_pos.values().map(|info| MAXIMUM_ROTATED_CUBE_VALUES[info[6].unwrap() as usize]).max().unwrap();
	}

	// same as make_encoding_recursive_loop(), but we've
	//   unrolled the loop here
	pub fn make_encoding_recursive(
			&self,
			start_cube_pos: isize,
			rotation: [usize; 6],
			included_cube_pos: &mut BTreeSet<isize>,
			best_encoding: u128,
			rotations_index: usize,
			mut offset: u8,
			mut encoding: u128) -> Option<(isize, u128, u8)> {
		let start_cube_info = self.cube_info_by_pos[&start_cube_pos];
		encoding = (encoding << 6) + (ROTATION_TABLE[start_cube_info[6].unwrap() as usize][rotations_index] as u128);
		// as soon as we can tell this is going to be an inferior encoding
		//   (smaller int value than the given best known encofing)
		//   we can stop right away
		if encoding < (best_encoding >> (offset * 6)) {
			return None;
		}
		let mut least_sig_cube_pos = start_cube_pos;
		included_cube_pos.insert(start_cube_pos);
		// direction 0
		match start_cube_info[rotation[0]] {
			Some(neighbor_pos) => {
				if !included_cube_pos.contains(&neighbor_pos) {
					match self.make_encoding_recursive(
							neighbor_pos,
							rotation,
							included_cube_pos,
							best_encoding,
							rotations_index,
							offset - 1,
							encoding) {
						Some((least_sig_cube_pos_new, encoding_ret, offset_ret)) => {
							least_sig_cube_pos = least_sig_cube_pos_new;
							encoding = encoding_ret;
							offset = offset_ret;
						}
						// if the Option is empty, that means we have determined
						//   somewhere deeper in the recursion that this is
						//   a dead-end inferior encoding, so we can stop
						None => { return None }
					}
				}
			}
			// if there is no neighbor in this direction just continue
			None => {}
		}
		// direction 1
		match start_cube_info[rotation[1]] {
			Some(neighbor_pos) => {
				if !included_cube_pos.contains(&neighbor_pos) {
					match self.make_encoding_recursive(
							neighbor_pos,
							rotation,
							included_cube_pos,
							best_encoding,
							rotations_index,
							offset - 1,
							encoding) {
						Some((least_sig_cube_pos_new, encoding_ret, offset_ret)) => {
							least_sig_cube_pos = least_sig_cube_pos_new;
							encoding = encoding_ret;
							offset = offset_ret;
						}
						// if the Option is empty, that means we have determined
						//   somewhere deeper in the recursion that this is
						//   a dead-end inferior encoding, so we can stop
						None => { return None }
					}
				}
			}
			// if there is no neighbor in this direction just continue
			None => {}
		}
		// direction 2
		match start_cube_info[rotation[2]] {
			Some(neighbor_pos) => {
				if !included_cube_pos.contains(&neighbor_pos) {
					match self.make_encoding_recursive(
							neighbor_pos,
							rotation,
							included_cube_pos,
							best_encoding,
							rotations_index,
							offset - 1,
							encoding) {
						Some((least_sig_cube_pos_new, encoding_ret, offset_ret)) => {
							least_sig_cube_pos = least_sig_cube_pos_new;
							encoding = encoding_ret;
							offset = offset_ret;
						}
						// if the Option is empty, that means we have determined
						//   somewhere deeper in the recursion that this is
						//   a dead-end inferior encoding, so we can stop
						None => { return None }
					}
				}
			}
			// if there is no neighbor in this direction just continue
			None => {}
		}
		// direction 3
		match start_cube_info[rotation[3]] {
			Some(neighbor_pos) => {
				if !included_cube_pos.contains(&neighbor_pos) {
					match self.make_encoding_recursive(
							neighbor_pos,
							rotation,
							included_cube_pos,
							best_encoding,
							rotations_index,
							offset - 1,
							encoding) {
						Some((least_sig_cube_pos_new, encoding_ret, offset_ret)) => {
							least_sig_cube_pos = least_sig_cube_pos_new;
							encoding = encoding_ret;
							offset = offset_ret;
						}
						// if the Option is empty, that means we have determined
						//   somewhere deeper in the recursion that this is
						//   a dead-end inferior encoding, so we can stop
						None => { return None }
					}
				}
			}
			// if there is no neighbor in this direction just continue
			None => {}
		}
		// direction 4
		match start_cube_info[rotation[4]] {
			Some(neighbor_pos) => {
				if !included_cube_pos.contains(&neighbor_pos) {
					match self.make_encoding_recursive(
							neighbor_pos,
							rotation,
							included_cube_pos,
							best_encoding,
							rotations_index,
							offset - 1,
							encoding) {
						Some((least_sig_cube_pos_new, encoding_ret, offset_ret)) => {
							least_sig_cube_pos = least_sig_cube_pos_new;
							encoding = encoding_ret;
							offset = offset_ret;
						}
						// if the Option is empty, that means we have determined
						//   somewhere deeper in the recursion that this is
						//   a dead-end inferior encoding, so we can stop
						None => { return None }
					}
				}
			}
			// if there is no neighbor in this direction just continue
			None => {}
		}
		// direction 5
		match start_cube_info[rotation[5]] {
			Some(neighbor_pos) => {
				if !included_cube_pos.contains(&neighbor_pos) {
					match self.make_encoding_recursive(
							neighbor_pos,
							rotation,
							included_cube_pos,
							best_encoding,
							rotations_index,
							offset - 1,
							encoding) {
						Some((least_sig_cube_pos_new, encoding_ret, offset_ret)) => {
							least_sig_cube_pos = least_sig_cube_pos_new;
							encoding = encoding_ret;
							offset = offset_ret;
						}
						// if the Option is empty, that means we have determined
						//   somewhere deeper in the recursion that this is
						//   a dead-end inferior encoding, so we can stop
						None => { return None }
					}
				}
			}
			// if there is no neighbor in this direction just continue
			None => {}
		}

		return Some((least_sig_cube_pos, encoding, offset));
	}

	// this is the original loop that was unrolled above
	//   in make_encoding_recursive()
	pub fn make_encoding_recursive_loop(
			&self,
			start_cube_pos: isize,
			rotation: [usize; 6],
			included_cube_pos: &mut BTreeSet<isize>,
			best_encoding: u128,
			rotations_index: usize,
			mut offset: u8,
			mut encoding: u128) -> Option<(isize, u128, u8)> {
		let start_cube_info = self.cube_info_by_pos[&start_cube_pos];
		encoding = (encoding << 6) + (ROTATION_TABLE[start_cube_info[6].unwrap() as usize][rotations_index] as u128);
		// as soon as we can tell this is going to be an inferior encoding
		//   (smaller int value than the given best known encofing)
		//   we can stop right away
		if encoding < (best_encoding >> (offset * 6)) {
			return None;
		}
		let mut least_sig_cube_pos = start_cube_pos;
		included_cube_pos.insert(start_cube_pos);
		for direction in rotation {
			match start_cube_info[direction] {
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
						Some((least_sig_cube_pos_new, encoding_ret, offset_ret)) => {
							least_sig_cube_pos = least_sig_cube_pos_new;
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
		return Some((least_sig_cube_pos, encoding, offset));
	}

	pub fn make_encoding(&self, start_cube_pos: isize, rotations_index: usize, best_encoding: u128) -> Option<(u128, isize)> {
		let mut included_cube_pos: BTreeSet<isize> = BTreeSet::new();
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
			Some((least_sig_cube_pos, encoding, _offset)) => {
				return Some((encoding, least_sig_cube_pos));
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
	pub fn find_canonical_info(&mut self, look_for_pos_as_least_significant: isize) -> &CanonicalInfo {
		if self.canonical_info.is_none() {
			let mut canonical = CanonicalInfo {
				enc: 0,
				least_significant_cube_pos: IMPOSSIBLE_POS,
				max_cube_value: self.find_maximum_cube_value()
			};
			let mut best_encoding: u128 = 0;
			let mut encoding_diff: u128;
			for (cube_pos, cube_info) in self.cube_info_by_pos.iter() {
				let cube_enc = cube_info[6].unwrap() as usize;
				// there could be more than one cube with the maximum rotated value
				if MAXIMUM_ROTATED_CUBE_VALUES[cube_enc] < canonical.max_cube_value {
					continue;
				}
				for rotations_index in MAXIMUM_CUBE_ROTATION_INDICES[cube_enc].iter() {
					match self.make_encoding(*cube_pos, *rotations_index as usize, best_encoding) {
						Some((encoding, least_significant_cube_pos)) => {
							encoding_diff = encoding - best_encoding;
							if encoding_diff > 0 {
								canonical.enc = encoding;
								canonical.least_significant_cube_pos = least_significant_cube_pos;
								best_encoding = encoding;
							// if we've found an equivalent encoding but where the
							//   tracked cube ends up in the least significant position,
							//   record the fact of that
							} else if encoding_diff == 0 && least_significant_cube_pos == look_for_pos_as_least_significant {
								canonical.least_significant_cube_pos = least_significant_cube_pos;
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

	// when we are looking for an encoding at least as large as the target,
	//   we can ignore all smaller encodings
	// this DOES NOT set the actual encoding for the polycube if smaller
	//   than the target, so it's only useful e.g. when checking if P+A-B=P
	pub fn find_canonical_enc_with_target(&mut self, target_encoding: u128) -> u128 {
		// leave default enc as 0 so we fail the P+A-B=P check if we
		//   don't find an encoding at least as large as the target
		let mut canonical = CanonicalInfo {
			enc: 0,
			least_significant_cube_pos: IMPOSSIBLE_POS,
			max_cube_value: self.find_maximum_cube_value()
		};
		let mut best_encoding: u128 = target_encoding;
		for (cube_pos, cube_info) in self.cube_info_by_pos.iter() {
			let cube_enc = cube_info[6].unwrap() as usize;
			// there could be more than one cube with the maximum rotated value
			if MAXIMUM_ROTATED_CUBE_VALUES[cube_enc] < canonical.max_cube_value {
				continue;
			}
			for rotations_index in MAXIMUM_CUBE_ROTATION_INDICES[cube_enc].iter() {
				match self.make_encoding(*cube_pos, *rotations_index as usize, best_encoding) {
					Some((encoding, least_significant_cube_pos)) => {
						if encoding >= best_encoding {
							canonical.enc = encoding;
							canonical.least_significant_cube_pos = least_significant_cube_pos;
							best_encoding = encoding;
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
		return canonical.enc;
	}
}

static mut N_COUNTS: [usize; 23] = [0; 23];


//  the initial delegator worker begins here
pub fn extend_and_delegate_outer(polycube: &mut Polycube, n: u8, atomic_halt: Arc<AtomicBool>,
		submit_queue: Arc<ArrayQueue<Polycube>>, response_queue: Arc<ArrayQueue<ThreadResponse>>, spawn_n: u8) {
	// thread-local random generator
	let mut rng = thread_rng();
	match extend_and_delegate(
			polycube,
			n,
			spawn_n,
			&submit_queue,
			&response_queue,
			&atomic_halt,
			&mut rng) {
		Some(found_counts_by_n) => {
			let mut all_n_counts: [usize; MAX_N] = [0; MAX_N];
			for i in 1..n+1 {
				match found_counts_by_n.get(i as usize) {
					Some(count) => {
						all_n_counts[i as usize] = *count;
					},
					None => {}
				}
			}
			match response_queue.push(ThreadResponse{ job_complete: true, results: Some(all_n_counts), polycube: None }) {
				Ok(_) => {}
				Err(_) => {
					panic!("response_queue.push() failed");
				}
			}
		}
		None => {
			// indicate that the initial delegator worker was
			//   halted with a special-case results=None and polycube=None here
			match response_queue.push(ThreadResponse{ job_complete: false, results: None, polycube: None }) {
				Ok(_) => {}
				Err(_) => {
					panic!("response_queue.push() failed");
				}
			}
		}
	}
}

pub fn extend_as_worker_outer(n: u8, atomic_halt: Arc<AtomicBool>, atomic_done: Arc<AtomicBool>,
		submit_queue: Arc<ArrayQueue<Polycube>>, response_queue: Arc<ArrayQueue<ThreadResponse>>) {
	let mut halted = false;
	// thread-local random generator
	let mut rng = thread_rng();
	while !halted {
		let polycube = submit_queue.pop();
		if polycube.is_none() {
			thread::sleep(Duration::from_millis(rng.gen_range(25..75)));
			// check if we've been halted or if we are done
			if atomic_halt.load(Ordering::Relaxed) || atomic_done.load(Ordering::Relaxed) {
				halted = true;
			//} else {
			//	thread::sleep(Duration::from_millis(100));
			}
			continue;
		}
		let mut polycube = polycube.unwrap();
		// save a copy of the original polycube so we can
		//   write it to disk if we are halted
		let polycube_orig_clone = polycube.copy();
		match extend_as_worker(
				&mut polycube,
				n,
				&submit_queue,
				&response_queue,
				&atomic_halt,
				&mut rng) {
			Some(found_counts_by_n) => {
				let mut all_n_counts: [usize; MAX_N] = [0; MAX_N];
				for i in 1..n+1 {
					match found_counts_by_n.get(i as usize) {
						Some(count) => {
							all_n_counts[i as usize] = *count;
						},
						None => {}
					}
				}
				match response_queue.push(ThreadResponse{ job_complete: true, results: Some(all_n_counts), polycube: None }) {
					Ok(_) => {}
					Err(_) => {
						panic!("response_queue.push() failed");
					}
				}
			}
			None => {
				// stopped due to the halt
				halted = true;
				match response_queue.push(ThreadResponse{ job_complete: false, results: None, polycube: Some(polycube_orig_clone) }) {
					Ok(_) => {}
					Err(_) => {
						panic!("response_queue.push() failed");
					}
				}
			}

		}
	}
	// after halt, drain the queue
	let mut found_something = true;
	while found_something {
		let polycube = submit_queue.pop();
		// put a message here to indicate that this Polycube is
		//   unevaluated
		if polycube.is_none() {
			found_something = false;
			continue;
		}
		match response_queue.push(ThreadResponse{ job_complete: false, results: None, polycube: polycube }) {
			Ok(_) => {}
			Err(_) => {
				panic!("response_queue.push() failed");
			}
		}
	}

	//except HaltSignal:
	//	# maybe indicate that the initial delegator worker was
	//	#   halted with a special-case None value here
	//	response_queue.put((False, None))
}

// expand the polycube until we reach n=delegate_at_n (spawn_n) and
//   and that point, place a .copy() of any found polycubes to
//   enumerate into the submit queue
pub fn extend_and_delegate(polycube: &Polycube, limit_n: u8, delegate_at_n: u8,
	submit_queue: &Arc<ArrayQueue<Polycube>>, response_queue: &Arc<ArrayQueue<ThreadResponse>>,
	atomic_halt: &Arc<AtomicBool>, rng: &mut ThreadRng) -> Option<[usize; 23]> {

	let mut found_counts_by_n: [usize; 23] = [0; 23];

	// we are done if we've reached the desired n,
	//   which we need to stop at because we are doing
	//   a depth-first recursive evaluation
	if polycube.n == limit_n {
		return Some(found_counts_by_n);
	}

	// keep a Set of all evaluated positions so we don't repeat them
	let mut tried_pos: BTreeSet<isize> = BTreeSet::new();
	tried_pos.extend(polycube.cube_info_by_pos.keys());

	let mut tried_canonicals: BTreeSet<u128> = BTreeSet::new();

	let mut canonical_try: &CanonicalInfo;
	let mut canonical_try_clone: CanonicalInfo;
	let mut least_significant_cube_pos: isize;

	let mut tmp_add = polycube.copy();
	let canonical_orig_enc: u128 = tmp_add.find_canonical_info(IMPOSSIBLE_POS).enc;

	let mut try_pos: isize;

	// if halt has been signalled, abandon the evaluation
	//   of this polycube
	// since this function is run many many times by each process/thread,
	//   we can greatly reduce use of AtomicBool.load() and increase per-
	//   process CPU utilization
	if rng.gen_range(0..1000) == 0 && atomic_halt.load(Ordering::Relaxed) {
		return None;
	}

	// for each cube, for each direction, add a cube
	for cube_pos in polycube.cube_info_by_pos.keys() {
		for direction_cost in DIRECTION_COSTS {
			try_pos = cube_pos + direction_cost;
			// skip if we've already tried this position
			if !tried_pos.insert(try_pos) {
				continue;
			}

			// create P+A
			tmp_add.add(try_pos);

			// skip if we've already seen some P+A with the same canonical representation
			//   (comparing the bitwise int only)
			canonical_try = tmp_add.find_canonical_info(try_pos);
			if !tried_canonicals.insert(canonical_try.enc) {
				tmp_add.remove(try_pos);
				continue;
			}

			least_significant_cube_pos = canonical_try.least_significant_cube_pos;

			// if try_pos (cube A) is the least significant, then P+A-A==P and P+A is a new unique polycube
			if least_significant_cube_pos == try_pos {
				found_counts_by_n[tmp_add.n as usize] += 1;
				// the initial delegator submits jobs for threads,
				//   but only if the found polycube has n=spawn_n
				if tmp_add.n == delegate_at_n {
					match submit_queue.push(tmp_add.copy()) {
						Ok(_) => {}
						Err(_) => {
							panic!("submit_queue.push() failed");
						}
					}
				} else {
					match extend_and_delegate(&mut tmp_add.copy(),
							limit_n, delegate_at_n,
							submit_queue, response_queue, atomic_halt, rng) {
						Some(futher_counts) => {
							for i in 1..limit_n+1 {
								found_counts_by_n[i as usize] += futher_counts[i as usize];
							}
						}
						// if we have detected a halt while running the recursion,
						//   we can continue to bubble the halt back up
						None => {
							return None;
						}
					}
				}
			} else {
				canonical_try_clone = canonical_try.clone();
				// remove the last of the ordered cubes (cube B) in P+A
				tmp_add.remove(least_significant_cube_pos);
				// if P+A-B has the same canonical representation as P, count P+A as a new unique polycube
				//   and continue recursion into that P+A
				if tmp_add.find_canonical_enc_with_target(canonical_orig_enc) == canonical_orig_enc {
					// replace the least significant cube we just removed
					tmp_add.add(least_significant_cube_pos);
					// replace the canonical info from before
					tmp_add.canonical_info = Some(canonical_try_clone);
					found_counts_by_n[tmp_add.n as usize] += 1;
					// the initial delegator submits jobs for threads,
					//   but only if the found polycube has n=spawn_n
					if tmp_add.n == delegate_at_n {
						match submit_queue.push(tmp_add.copy()) {
							Ok(_) => {}
							Err(_) => {
								panic!("submit_queue.push() failed");
							}
						}
					} else {
						match extend_and_delegate(&mut tmp_add.copy(),
								limit_n, delegate_at_n,
								submit_queue, response_queue, atomic_halt, rng) {
							Some(futher_counts) => {
								for i in 1..limit_n+1 {
									found_counts_by_n[i as usize] += futher_counts[i as usize];
								}
							}
							// if we have detected a halt while running the recursion,
							//   we can continue to bubble the halt back up
							None => {
								return None;
							}
						}
					}

				// undo the temporary removal of the least significant cube,
				//   but only if it's not the same as the cube we just tried
				//   since we remove that one before going to the next iteration
				//   of the loop
				} else {
					tmp_add.add(least_significant_cube_pos);
				}
			}

			// revert creating P+A to try adding a cube at another position
			tmp_add.remove(try_pos);
		}
	}
	return Some(found_counts_by_n);
}

// same as extend_single_thread, but
//   - we report counts to the results queue
//   - we occasionally check for a halt signal
pub fn extend_as_worker(polycube: &mut Polycube, limit_n: u8,
		submit_queue: &Arc<ArrayQueue<Polycube>>, response_queue: &Arc<ArrayQueue<ThreadResponse>>,
		atomic_halt: &Arc<AtomicBool>, rng: &mut ThreadRng) -> Option<[usize; 23]> {

	let mut found_counts_by_n: [usize; 23] = [0; 23];

	//found_counts_by_n[polycube.n as usize] += 1;

	// we are done if we've reached the desired n,
	//   which we need to stop at because we are doing
	//   a depth-first recursive evaluation
	if polycube.n == limit_n {
		return Some(found_counts_by_n);
	}

	// keep a Set of all evaluated positions so we don't repeat them
	let mut tried_pos: BTreeSet<isize> = BTreeSet::new();

	let mut tried_canonicals: BTreeSet<u128> = BTreeSet::new();

	let canonical_orig_enc: u128 = polycube.find_canonical_info(IMPOSSIBLE_POS).enc;
	let mut canonical_try: &CanonicalInfo;
	let mut canonical_try_clone: CanonicalInfo;
	let mut least_significant_cube_pos: isize;

	let mut try_pos: isize;

	// if halt has been signalled, abandon the evaluation
	//   of this polycube
	// since this function is run many many times by each process/thread,
	//   we can greatly reduce use of AtomicBool.load() and increase per-
	//   process CPU utilization
	if rng.gen_range(0..1000) == 0 && atomic_halt.load(Ordering::Relaxed) {
		return None;
	}

	// for each cube, for each direction, add a cube
	// create a list to iterate over because the dict will change
	//   during recursion within the loop
	let original_positions: Vec<isize> = polycube.cube_info_by_pos.keys().cloned().collect();
	// include all existing cubes' positions in the tried_pos set
	tried_pos.extend(original_positions.iter());
	for cube_pos in original_positions {
		for direction_cost in DIRECTION_COSTS {
			try_pos = cube_pos + direction_cost;
			// skip if we've already tried this position
			if !tried_pos.insert(try_pos) {
				continue;
			}

			// create P+A
			polycube.add(try_pos);

			// skip if we've already seen some p+1 with the same canonical representation
			//   (comparing the bitwise int only)
			canonical_try = polycube.find_canonical_info(try_pos);
			if !tried_canonicals.insert(canonical_try.enc) {
				polycube.remove(try_pos);
				continue;
			}

			least_significant_cube_pos = canonical_try.least_significant_cube_pos;

			// if try_pos (cube A) is the least significant, then P+A-A==P and P+A is a new unique polycube
			if least_significant_cube_pos == try_pos {
				found_counts_by_n[polycube.n as usize] += 1;
				match extend_as_worker(polycube, limit_n,
						submit_queue, response_queue, atomic_halt, rng) {
					Some(futher_counts) => {
						for i in 1..limit_n+1 {
							found_counts_by_n[i as usize] += futher_counts[i as usize];
						}
					}
					// if we have detected a halt while running the recursion,
					//   we can continue to bubble the halt back up
					None => {
						return None;
					}
				}
			} else {
				canonical_try_clone = canonical_try.clone();
				// remove the last of the ordered cubes (cube B) in P+A
				polycube.remove(least_significant_cube_pos);
				// if P+A-B has the same canonical representation as P, count P+A as a new unique polycube
				//   and continue recursion into that P+A
				if polycube.find_canonical_enc_with_target(canonical_orig_enc) == canonical_orig_enc {
					// replace the least significant cube we just removed
					polycube.add(least_significant_cube_pos);
					// replace the canonical info from before
					polycube.canonical_info = Some(canonical_try_clone);
					found_counts_by_n[polycube.n as usize] += 1;
					// continue recursion
					match extend_as_worker(polycube, limit_n,
							submit_queue, response_queue, atomic_halt, rng) {
						Some(futher_counts) => {
							for i in 1..limit_n+1 {
								found_counts_by_n[i as usize] += futher_counts[i as usize];
							}
						}
						// if we have detected a halt while running the recursion,
						//   we can continue to bubble the halt back up
						None => {
							return None;
						}
					}

				// undo the temporary removal of the least significant cube,
				//   but only if it's not the same as the cube we just tried
				//   since we remove that one before going to the next iteration
				//   of the loop
				} else {
					polycube.add(least_significant_cube_pos);
				}
			}

			// revert creating P+A to try adding a cube at another position
			polycube.remove(try_pos);
		}
	}
	return Some(found_counts_by_n);
}

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
	let mut tried_pos: BTreeSet<isize> = BTreeSet::new();

	let mut tried_canonicals: BTreeSet<u128> = BTreeSet::new();

	let canonical_orig_enc: u128 = polycube.find_canonical_info(IMPOSSIBLE_POS).enc;
	let mut canonical_try: &CanonicalInfo;
	let mut canonical_try_clone: CanonicalInfo;
	let mut least_significant_cube_pos: isize;

	let mut try_pos: isize;

	// for each cube, for each direction, add a cube
	// create a list to iterate over because the dict will change
	//   during recursion within the loop
	let original_positions: Vec<isize> = polycube.cube_info_by_pos.keys().cloned().collect();
	// include all existing cubes' positions in the tried_pos set
	tried_pos.extend(original_positions.iter());
	for cube_pos in original_positions {
		for direction_cost in DIRECTION_COSTS {
			try_pos = cube_pos + direction_cost;
			// skip if we've already tried this position
			if !tried_pos.insert(try_pos) {
				continue;
			}

			// create P+A
			polycube.add(try_pos);

			// skip if we've already seen some P+A with the same canonical representation
			//   (comparing the bitwise int only)
			canonical_try = polycube.find_canonical_info(try_pos);
			if !tried_canonicals.insert(canonical_try.enc) {
				polycube.remove(try_pos);
				continue;
			}

			least_significant_cube_pos = canonical_try.least_significant_cube_pos;

			// if try_pos (cube A) is the least significant, then P+A-A==P and P+A is a new unique polycube
			if least_significant_cube_pos == try_pos {
				extend_single_thread(polycube,  limit_n, depth+1);
			} else {
				canonical_try_clone = canonical_try.clone();
				// remove the last of the ordered cubes (cube B) in P+A
				polycube.remove(least_significant_cube_pos);
				// if P+A-B has the same canonical representation as P, count it as a new unique polycube
				//   and continue recursion into that P+A
				if polycube.find_canonical_enc_with_target(canonical_orig_enc) == canonical_orig_enc {
					// replace the least significant cube we just removed
					polycube.add(least_significant_cube_pos);
					// replace the canonical info from before
					polycube.canonical_info = Some(canonical_try_clone);
					// continue recursion
					extend_single_thread(polycube,  limit_n, depth+1);

				// undo the temporary removal of the least significant cube,
				//   but only if it's not the same as the cube we just tried
				//   since we remove that one before going to the next iteration
				//   of the loop
				} else {
					polycube.add(least_significant_cube_pos);
				}
			}

			// revert creating P+A to try adding a cube at another position
			polycube.remove(try_pos);
		}
	}
}

pub fn write_resume_file(n: u8, spawn_n: u8, polycubes_to_write_to_disk: Vec<Polycube>, elapsed_sec: f64) {
	let timestamp = Local::now().to_rfc3339().replace('-', "").replace(':', "");
	let filename = format!("halt-n{}-{}.txt.gz", n, &timestamp[0..15]);
	let resume_file_path = create_executable_sibling_file(filename.as_str());
	println!("writing {} polycubes to [{}]...", polycubes_to_write_to_disk.len(), resume_file_path.to_str().unwrap());
	let mut file_buf = File::create(resume_file_path).unwrap();
	// i am getting strange repeated/missing characters in the .gz file,
	//   so i am trying a lower compression level
	//let mut gz = GzEncoder::new(&mut file_buf, Compression::best());
	// still getting errors at the default compression level
	//let mut gz = GzEncoder::new(&mut file_buf, Compression::default());
	// still getting errors at the fast compression level
	//let mut gz = GzEncoder::new(&mut file_buf, Compression::fast());
	let mut gz = GzEncoder::new(&mut file_buf, Compression::none());
	match gz.write(format!("{}\n{}\n{}\n", n, spawn_n, elapsed_sec).as_bytes()) {
		Ok(_) => {}
		Err(err) => {
			println!("error writing to resume file:\n{}", err);
			// if we have an error writing resume file, there's no
			//   point in continuing
			exit(1);
		}
	}
	unsafe {
		match gz.write(format!("{}\n", N_COUNTS.iter().enumerate().map(|(i,count)| format!("{}={}", i, count)).collect::<Vec<String>>().join(",")).as_bytes()) {
			Ok(_) => {}
			Err(err) => {
				println!("error writing to resume file:\n{}", err);
				// if we have an error writing resume file, there's no
				//   point in continuing
				exit(1);
			}
		}
	}
	for polycube in polycubes_to_write_to_disk.iter() {
		let cubes = polycube.cube_info_by_pos.keys().map(|pos| match pos.to_string().as_str() {
			"--401" => {
				panic!("the postion {} is generating a string of --401", pos);
			}
			_ => {
				if pos.to_string().starts_with("--") {
					panic!("the postion {} is generating a string of {}", pos, pos.to_string());
				}
				pos.to_string()
			}
		}).collect::<Vec<String>>().join(",");
		match gz.write(format!("{}\n", cubes).as_bytes()) {
			Ok(_) => {}
			Err(err) => {
				println!("error writing to resume file:\n{}", err);
				// if we have an error writing resume file, there's no
				//   point in continuing
				exit(1);
			}
		}
	}
	match gz.write("--end--".as_bytes()) {
		Ok(_) => {}
		Err(err) => {
			println!("error writing to resume file:\n{}", err);
			// if we have an error writing resume file, there's no
			//   point in continuing
			exit(1);
		}
	}
	gz.flush().unwrap();
}

pub fn read_resume_file(resume_file_path: &PathBuf)
		-> (u8, u8, BTreeMap<u8, usize>, f64, Vec<Vec<isize>>) {
	let file_err_msg: String = format!("error reading resume file [{}]", resume_file_path.to_string_lossy());
	let f = File::open(resume_file_path).expect(file_err_msg.as_str());
	let mut buf = BufReader::new(GzDecoder::new(f));
	let mut n: u8 = 0;
	let mut spawn_n: u8 = 0;
	let mut previous_total_elapsed_sec: f64 = 0.0;
	let mut n_counts: BTreeMap<u8, usize> = BTreeMap::new();
	let mut polycubes_read: Vec<Vec<isize>> = Vec::new();
	let mut line: String = String::new();
	let mut line_num: usize = 0;
	let mut len: usize = 1;
	while len > 0 {
		line.clear();
		len = buf.read_line(&mut line).expect(file_err_msg.as_str());
		line = line.trim().to_string();
		if line == "--end--" {
			break;
		}
		if line_num == 0 {
			//println!("using line [{}] as n", line);
			// first line is the arg_n
			n = line.parse().unwrap();
		} else if line_num == 1 {
			//println!("using line [{}] as spawn_n", line);
			// second line is the arg_spawn_n
			spawn_n  = line.parse().unwrap();
		} else if line_num == 2 {
			//println!("using line [{}] as previous_total_elapsed_sec", line);
			// third line is the previous_total_elapsed_sec
			previous_total_elapsed_sec = line.parse().unwrap();
		} else if line_num == 3 {
			//println!("using line [{}] as n_counts", line);
			// fourth line is the n_counts: "1=1,2=1,3=2,..."
			for item in line.split(',')
					.map(|item| item.split_once('=')) {
				let (n, count) = item.unwrap();
				if count != "0" {
					println!("    n = {: >2}: {}", n, count);
				}
				n_counts.insert(n.parse().unwrap(), count.parse().unwrap());
			}
		} else if line_num > 3 && line.len() > 0 {
			// lines 5 and beyond are polycubes' cube positions, one polycube per line
			let cubes = line.split(',')
				.map(|cube_pos| match cube_pos.parse::<isize>(){
					Ok(pos) => pos,
					Err(_) => {
						println!("error: invalid cube position [{}] in resume file at line {}" , cube_pos, line_num+1);
						exit(1);
					}
				}).collect();
			polycubes_read.push(cubes);
		}
		line_num += 1;
	}
	println!("restored {} polycubes for submit queue", polycubes_read.len());
	return (n, spawn_n, n_counts, previous_total_elapsed_sec, polycubes_read);
}

pub fn create_executable_sibling_file(filename: &str) -> PathBuf {
	return match env::current_exe() {
		Ok(executable_path) => {
			executable_path.parent().unwrap().join(filename)
		}
		Err(_) => {
			println!("error: could not determine current executable path");
			exit(1);
		}
	};
}

pub fn seconds_to_dur(s: f64) -> String {
	let days = (s / 86400.0).floor();
	let hours = ((s - (days * 86400.0)) / 3600.0).floor();
	let minutes = ((s - (days * 86400.0) - (hours * 3600.0)) / 60.0).floor();
	let seconds = s - (days * 86400.0) - (hours * 3600.0) - (minutes * 60.0);
	let fsec = format!("{}{:.3}", if seconds < 10.0 { "0" } else { "" }, seconds);
	if days > 0.0 {
		return format!("{} days {:0>2}h:{:0>2}m:{}s", days, hours, minutes, fsec);
	}
	return format!("{:0>2}h:{:0>2}m:{}s", hours, minutes, fsec);
}

pub fn validate_resume_file(resume_file: &str) -> Result<PathBuf, String> {
    let resume_file_path = PathBuf::from(resume_file);
    if !resume_file_path.is_file() {
        return Err(format!("<resume-file> [{}] is not a regular file, does not exist, or does not have permissions \
            necessary for access", resume_file));
    }
    Ok(resume_file_path)
}

pub fn print_results(complete: bool, n: u8) {
	unsafe {
		println!("\n\n{}results:", if complete { "" } else { "partial " });
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
	-  <resume-file>: a .txt.gz file previously created by this program",
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
		} else if args[cursor] == "--n" || args[cursor] == "-n" {
			arg_n = match args[cursor + 1].parse() {
				Ok(n) => {
					if n < 2 {
						println!("error: n must be greater than 1");
						println!("{}", usage);
						exit(1);
					} else if n > 21 {
						println!("error: n greater than 21 not yet ;) supported");
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
		} else if args[cursor] == "--threads" || args[cursor] == "-t" {
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
		} else if args[cursor] == "--spawn-n" || args[cursor] == "-s" {
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
		} else if args[cursor] == "--resume-from-file" || args[cursor] == "-r" {
			arg_resume_file = match validate_resume_file(&args[cursor + 1]) {
				Ok(path) => Some(path),
				Err(err) => {
					println!("error: {}", err);
					println!("{}", usage);
					exit(1);
				}
			};
		} else {
			println!("error: unknown argument [{}]", args[cursor]);
			println!("{}", usage);
			exit(1);
		}
		cursor += 2;
	}
	// we either need a <resume-file> or a value for <n>
	match arg_resume_file.as_ref() {
		Some(resume_file_path) => {
			println!("resuming from file: {}", resume_file_path.to_string_lossy());
		}
		None => {
			if arg_n == 0 {
				println!("error: n must be specified");
				println!("{}", usage);
				exit(1);
			}
		}
	}

	let halt_file_path = create_executable_sibling_file("halt-signal.txt");
	if halt_file_path.exists() {
		println!("found halt file [{}] already exists, stopping...", halt_file_path.to_str().unwrap());
		exit(0);
	}

	let mut previous_total_elapsed_sec: f64 = 0.0;
	let mut complete = false;
	let start_time = Instant::now();
	let mut last_count_increment_time: Option<Instant> = None;
	if arg_threads == 0 {
		extend_single_thread(&mut Polycube::new(true), arg_n, 0);
		complete = true;
	} else {
		let polycubes_to_resume: Vec<Vec<isize>> = match arg_resume_file.as_ref() {
			Some(resume_file_path) => {
				let (
					resume_n,
					resume_spawn_n,
					resume_n_counts,
					resume_total_elapsed_sec,
					polycubes_read
				) = read_resume_file(resume_file_path);
				arg_n = resume_n;
				arg_spawn_n = resume_spawn_n;
				previous_total_elapsed_sec = resume_total_elapsed_sec;
				unsafe {
					for (i, count) in resume_n_counts.iter() {
						N_COUNTS[*i as usize] = *count as usize;
					}
				}
				polycubes_read
			}
			None => {
				Vec::new()
			}
		};

		let mut saved_worker_jobs: usize = 0;
		let total_worker_jobs = WELL_KNOWN_N_COUNTS[arg_spawn_n as usize];
		let mut compl_worker_jobs: isize = match arg_resume_file.as_ref() {
			Some(_path) => {
				(total_worker_jobs - polycubes_to_resume.len()).try_into().unwrap()
			}
			None => 0
		};
		println!("to halt early, create the file [{}]", halt_file_path.to_str().unwrap());
		let mut polycubes_to_write_to_disk: Vec<Polycube> = Vec::new();

		// these are the found canonical Polycubes of whatever
		//   size that the child processes will evaluate
		// we know that there will not be more Polycubes submitted
		//   than the number of unique polycubes with n=<spawn-n>
		let submit_queue: Arc<ArrayQueue<Polycube>> =
			Arc::new(ArrayQueue::new(total_worker_jobs));
		// the child processes will return both counts for fully-
		//   evaluated Polycubes and also non-evaluated Polycubes
		//   to write to disk for continuing later
		// we know that there will not be more responses than
		//   polycubes submitted, so we can use the same bounding size
		//   as submit_queue
		let response_queue: Arc<ArrayQueue<ThreadResponse>> =
			Arc::new(ArrayQueue::new(total_worker_jobs));


		// bool for signalling that an early halt has been requested
		let atomic_halt = Arc::new(AtomicBool::new(false));
		// bool for signalling to the workers to stop looking for jobs
		let atomic_done = Arc::new(AtomicBool::new(false));

		let mut initial_workers_to_spawn = arg_threads;
		if arg_resume_file.as_ref().is_none() {
			// initially spawn threads-1 worker threads, plus one
			//   thread for the initial work delegator
			initial_workers_to_spawn -= 1
		}
		let delegator_proc: Option<JoinHandle<()>> = match arg_resume_file.as_ref() {
			Some(_path) => {
				for cubes in polycubes_to_resume.iter() {
					let mut polycube = Polycube::new(false);
					for cube_pos in cubes {
						polycube.add(*cube_pos);
					}
					match submit_queue.push(polycube) {
						Ok(_) => {}
						Err(_) => {
							println!("error: could not push polycube to submit queue");
							exit(1);
						}
					}
				}
				None
			}
			None => {
				unsafe {
					N_COUNTS[1] = 1;
				}
				let ah = atomic_halt.clone();
				let sq = submit_queue.clone();
				let rq = response_queue.clone();
				let handle = thread::spawn(move || {
					let mut polycube: Polycube = Polycube::new(true);
					extend_and_delegate_outer(&mut polycube, arg_n, ah, sq, rq, arg_spawn_n);
				});
				Some(handle)
			}
		};
		let mut worker_handles: Vec<JoinHandle<()>> = Vec::new();
		for _i in 0..initial_workers_to_spawn {
			let ah = atomic_halt.clone();
			let ad = atomic_done.clone();
			let sq = submit_queue.clone();
			let rq = response_queue.clone();
			let handle = thread::spawn(move || {
				extend_as_worker_outer(arg_n, ah, ad, sq, rq);
			});
			worker_handles.push(handle);
		}
		let mut halted = false;
		let mut last_stats_and_halt = Instant::now();
		while !halted || !submit_queue.is_empty() || !response_queue.is_empty() {
			// once the initial work delegator has finished,
			//   spawn a new worker thread
			if !halted && arg_resume_file.as_ref().is_none()
					&& delegator_proc.as_ref().unwrap().is_finished() && worker_handles.len() < arg_threads as usize {
				println!("\ninitial delegator thread has finished, spawning a new worker thread");
				// the initial delegator thread submits its results through
				//   the same response_queue as the rest of the workers, but it shouldn't
				//   be counted as a completed worker job
				compl_worker_jobs -= 1;
				let ah = atomic_halt.clone();
				let ad = atomic_done.clone();
				let sq = submit_queue.clone();
				let rq = response_queue.clone();
				let handle = thread::spawn(move || {
					extend_as_worker_outer(arg_n, ah, ad, sq, rq);
				});
				worker_handles.push(handle);
			}
			// check for halt file
			if !halted {
				if halt_file_path.exists() {
					println!("\nfound halt file [{}], stopping...", halt_file_path.to_str().unwrap());
					// signal to the threads that they should stop
					atomic_halt.store(true, Ordering::Relaxed);
					halted = true;
				}
			}
			// check if everything has completed
			if !halted && (delegator_proc.is_none() || delegator_proc.as_ref().unwrap().is_finished())
					&& submit_queue.is_empty() {
				println!("\nlooks like we have finished!  stopping...");
				// signal to the threads that they should stop
				atomic_done.store(true, Ordering::Relaxed);
				halted = true;
				complete = true;
			}

			// regardless of halt file, continue to process responses
			//   sent by the workers (if any are currently available)
			let mut found_something = true;
			while found_something {
				if last_stats_and_halt.elapsed().as_secs_f32() > 1.0 {
					last_stats_and_halt = Instant::now();
					// check for halt file
					if !halted && !complete && halt_file_path.exists() {
						println!("\nfound halt file [{}], stopping...", halt_file_path.to_str().unwrap());
						// signal to the threads that they should stop
						atomic_halt.store(true, Ordering::Relaxed);
						halted = true;
					}
					// print stats
					if compl_worker_jobs > 0 {
						let time_elapsed = start_time.elapsed();
						let seconds_per_thread = (time_elapsed.as_secs_f64() + previous_total_elapsed_sec) / (compl_worker_jobs as f64);
						let threads_remaining = (total_worker_jobs as isize - compl_worker_jobs) as f64;
						let seconds_remaining = threads_remaining * seconds_per_thread;
						let pct_complete = (compl_worker_jobs as f64 * 100.0) / (total_worker_jobs as f64);
						let total_seconds = seconds_remaining + time_elapsed.as_secs_f64() + previous_total_elapsed_sec;
						print!("    {:.4}% complete, ETA:[{}], total:[{}], counting for n={}:[{}], outstanding threads:[{}-{}={}]        \r",
							pct_complete,
							seconds_to_dur(seconds_remaining),
							seconds_to_dur(total_seconds),
							arg_n,
							unsafe { N_COUNTS[arg_n as usize] },
							total_worker_jobs,
							compl_worker_jobs,
							total_worker_jobs as isize - compl_worker_jobs);
						std::io::stdout().flush().unwrap();
					}
				}
				let response = response_queue.pop();
				if response.is_none() {
					found_something = false;
					continue;
				}
				let response = response.unwrap();
				if response.job_complete {
					// we have a completed job
					//   (either a fully-evaluated polycube or a non-evaluated polycube)
					//   so we can increment the completed worker jobs count
					compl_worker_jobs += 1;
					// if we have a fully-evaluated polycube, we can increment the
					//   count for that polycube's n
					if response.results.is_some() {
						let results = response.results.unwrap();
						unsafe {
							for i in 1..arg_n+1 {
								N_COUNTS[i as usize] += results[i as usize];
							}
						}
					}
					last_count_increment_time = Some(Instant::now());
				} else {
					match response.polycube {
						// we have a non-evaluated polycube, so we can write it to disk
						Some(polycube) => {
							polycubes_to_write_to_disk.push(polycube);
							saved_worker_jobs += 1;
						}
						None => {
							println!("\nthe initial delegator worker was halted.  thus, we cannot resume from this point later and will not write any data to disk")
						}
					}
				}
			}
			thread::sleep(Duration::from_millis(1000));
		}
		// we probably don't need to join, but it would be nice to
		//   figure out how to do that here
		for w in worker_handles.into_iter() {
			w.join().unwrap();
		}
		println!("\nall threads have completed: compl_worker_jobs={}, saved_worker_jobs={} (compl+saved={}, where @n={} should be total_worker_jobs={})",
			compl_worker_jobs, saved_worker_jobs, compl_worker_jobs + saved_worker_jobs as isize, arg_spawn_n, total_worker_jobs);
		if polycubes_to_write_to_disk.len() > 0 {
			write_resume_file(
				arg_n,
				arg_spawn_n,
				polycubes_to_write_to_disk,
				previous_total_elapsed_sec + (last_count_increment_time.unwrap().duration_since(start_time).as_secs_f64()),
			)
		}
	}
	if last_count_increment_time.is_none() {
		last_count_increment_time = Some(Instant::now());
	}
	print_results(complete, arg_n);
	if last_count_increment_time.is_none() {
		last_count_increment_time = Some(Instant::now());
	}
	let last_count_increment_time = last_count_increment_time.unwrap();
	let time_elapsed = last_count_increment_time.duration_since(start_time);
	if arg_resume_file.as_ref().is_none() {
		println!("elapsed seconds: {}.{}", time_elapsed.as_secs(), time_elapsed.subsec_micros());
	} else {
		let total_time_elapsed = time_elapsed.as_secs_f64() + previous_total_elapsed_sec;
		println!("elapsed seconds: {:.6} + {:.6} (previously) = {:.6}", time_elapsed.as_secs_f64(), previous_total_elapsed_sec, total_time_elapsed);
	}

}
