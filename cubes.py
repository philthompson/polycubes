# python3
#

from copy import deepcopy
from functools import reduce
import signal
import sys
import time

def int_to_bit_list_unbound(n):
	num_bits = 1
	while 2 ** num_bits <= n:
		num_bits += 1
	return [1 if n & (1 << (num_bits-1-i)) else 0 for i in range(num_bits)]

# 5 -> [0, 0, 0, 1, 0, 1]
# bitwise & for each position from 0-6
# thanks to https://stackoverflow.com/a/30971101/259456
def int_to_bit_list(n):
	return [1 if n & (1 << (5-i)) else 0 for i in range(6)]

# apply rotation: grab the ith bit from the list
def rotate_bit_list(bits, rot):
	return [bits[i] for i in rot]

# [0, 0, 0, 1, 0, 1] -> 5
def bit_list_to_int(bits):
	# nice one-liner but it could be slower than a loop
	#return sum([v << i for i,v in enumerate(reversed(bits))])
	sum = 0
	for i,v in enumerate(reversed(bits)):
		sum += v << i
	return sum

def rotate_value(value, rotation):
	return bit_list_to_int(rotate_bit_list(int_to_bit_list(value), rotation))

# minus x, plus x, minus y, plus y, minus z, plus z
directions = [0, 1, 2, 3, 4, 5]
# used to create a unique integer position for each cube
#   in a polycube
direction_costs = [-1, 1, -100, 100, -10_000, 10_000]
# each of the 24 possible rotations of a 3d object
#   (where each value refers to one of the above directions)
rotations = [[0,1,2,3,4,5], [0,1,3,2,5,4], [0,1,4,5,3,2], [0,1,5,4,2,3],\
             [1,0,2,3,5,4], [1,0,3,2,4,5], [1,0,4,5,2,3], [1,0,5,4,3,2],\
             [2,3,0,1,5,4], [2,3,1,0,4,5], [2,3,4,5,0,1], [2,3,5,4,1,0],\
             [3,2,0,1,4,5], [3,2,1,0,5,4], [3,2,4,5,1,0], [3,2,5,4,0,1],\
             [4,5,0,1,2,3], [4,5,1,0,3,2], [4,5,2,3,1,0], [4,5,3,2,0,1],\
             [5,4,0,1,3,2], [5,4,1,0,2,3], [5,4,2,3,0,1], [5,4,3,2,1,0]]

rotation_table = {}
# for each possible grouping of presence (1) or absence (0) of a cube's 6 neighbors (2^6=64 possibilities)
for cube_enc in range(0, 64):
	# apply each of the 24 possible rotations of a 3d object
	#rotation_table[cube_enc] = [bit_list_to_int(rotate_bit_list(int_to_bit_list(cube_enc), rotation)) for rotation in rotations]
	# same as above, just moved the list comprehension to a function
	rotation_table[cube_enc] = [rotate_value(cube_enc, rotation) for rotation in rotations]

maximum_rotated_cube_values = {}
# for each possible grouping of presence (1) or absence (0) of a cube's 6 neighbors (2^6=64 possibilities)
for cube_enc,cube_rotations in rotation_table.items():
	# find the maximum numeric value of its 24 possible rotations
	maximum_rotated_cube_values[cube_enc] = max(cube_rotations)

# find the "rotations" indices that result in the maximum value for each cube_enc
maximum_cube_rotation_indices = {}
for cube_enc,max_value in maximum_rotated_cube_values.items():
	maximum_cube_rotation_indices[cube_enc] = []
	# not sure why enumerate(rotations) doesn't work here
	for i,rotation in enumerate(rotations):
		if rotate_value(cube_enc, rotation) == max_value:
			maximum_cube_rotation_indices[cube_enc].append(i)
	#i = 0
	#for rotation in enumerate(rotations):
	#	if rotate_value(cube_enc, rotation) == max_value:
	#		maximum_cube_rotation_indices[cube_enc].append(i)
	#	i += 1

# count of unique polycubes of size n
n_counts = [0 for i in range(22)]

class Cube:
	# integer encoding for this cube's neighbors:
	# 6 bits per cube, where a 1 represents a present neighbor in that direction
	enc = 0
	# references to neighbor Cube instances
	#neighbors = [None, None, None, None, None, None]
	neighbors = None
	# position in the polycube defined as (x + 100y + 10,000z)
	pos	= 0
	# used when encoding a polycube
	temp = 0
	# for debugging
	coords = None

	def __init__(self, *, pos):
		self.pos = pos
		self.neighbors = [None, None, None, None, None, None]
		self.temp = 0
		self.enc = 0
		self.coords = {'x':0, 'y':0, 'z':0}

	def copy(self):
		new_cube = Cube(pos=self.pos)
		new_cube.enc = self.enc
		new_cube.neighbors = self.neighbors.copy()
		new_cube.temp = self.temp
		new_cube.coords = self.coords.copy()
		return new_cube

class Polycube:

	# number of cubes in this polycube
	n = 0
	#enc = None
	# positions of cubes in this polycube
	cubes = {}
	# used when encoding this polycube
	temp = 0

	canonical_info = None

	# initialize with 1 cube at (0, 0, 0)
	def __init__(self, create_initial_cube):
		self.canonical_info = None
		if create_initial_cube:
			self.n = 1
			self.cubes[0] = Cube(pos=0)
		else:
			self.n = 0
			self.cubes = {}

	def copy(self):
		new_polycube = Polycube(create_initial_cube=False)
		new_polycube.n = self.n
		#new_polycube.cubes = self.cubes.copy()
		new_polycube.cubes = {}
		for pos,cube in self.cubes.items():
			new_polycube.cubes[pos] = cube.copy()
		# thanks to https://stackoverflow.com/a/15214597/259456
		new_polycube.canonical_info = deepcopy(self.canonical_info)
		return new_polycube

	# where enc is an encoding of the polycube with
	#   6 bits per cube
	#def __init__(self, *, enc):
	#	self.enc = enc

	def add(self, *, pos):
		global directions
		new_cube = Cube(pos=pos)
		self.cubes[pos] = new_cube
		self.n += 1
		self.canonical_info = None

		# update each of our cube's enc values for the default
		#   rotation of [0,1,2,3,4,5]
		# set the neighbors for the new cube and set it as a neighbor to those cubes
		for direction in directions:
			# neighbor cube position in the direction
			dir_pos = pos + direction_costs[direction]
			dir_cube = self.cubes.get(dir_pos)
			# if there is no neightbor cube in this direction, continue to next direction
			if dir_cube is None:
				continue
			# sanity check to see if any cube has more neighbors than
			#   the polycube has cubes
			#if self.n < reduce(lambda acc, cur: acc + (1 if cur is not None else 0), dir_cube.neighbors, 0):
			#	pass
			new_cube.neighbors[direction] = dir_cube
			# we use rotation of [0,1,2,3,4,5] where the '0'
			#   direction is -x and is the most significant bit
			#   in each cube's .enc value, so we need '0' to
			#   cause a left shift by 5 bits
			# try bitwise OR instead of adding (but the original javascript
			#   implementation used addition here)
			#new_cube.enc += (1 << (5-direction))
			new_cube.enc |= (1 << (5-direction))
			#if new_cube.enc > 63:
			#	pass
			#if new_cube.enc < 0:
			#	pass
			# use XOR to flip between each direction and its opposite
			#   to set the neighbor's neighbor to the new cube
			#   (0<->1, 2<->3, 4<->5)
			dir_cube.neighbors[direction ^ 1] = new_cube
			#if False:
			#	dir_cube_enc_prev = dir_cube.enc
			#	flipped_dir = direction ^ 1
			#	to_add_to_enc = 1 << flipped_dir
			#	dir_cube_enc_new = dir_cube_enc_prev + to_add_to_enc
			# we use rotation of [0,1,2,3,4,5] where the '0'
			#   direction is -x and is the most significant bit
			#   in each cube's .enc value, so we need '0' to
			#   cause a left shift by 5 bits (and here we use
			#   XOR to flip to the opposite direction)
			# try bitwise OR instead of adding (but the original javascript
			#   implementation used addition here)
			#dir_cube.enc += (1 << ((5-direction) ^ 1))
			dir_cube.enc |= (1 << ((5-direction) ^ 1))
			#if dir_cube.enc > 63:
			#	pass
			#if dir_cube.enc < 0:
			#	pass

	def remove(self, *, pos):
		# remove this cube from each of its neighbors
		for dir,neighbor in enumerate(self.cubes[pos].neighbors):
			if neighbor is None:
				continue
			neighbor_enc_orig = neighbor.enc
			#if neighbor_enc_orig == 0:
			#	pass
			# i'm doing something wrong so i'll just use pos
			#   to look up the neighbor cube instance that way
			#neighbor.neighbors[dir ^ 1] = None
			self.cubes[neighbor.pos].neighbors[dir ^ 1] = None
			# we use rotation of [0,1,2,3,4,5] where the '0'
			#   direction is -x and is the most significant bit
			#   in each cube's .enc value, so we need '0' to
			#   cause a left shift by 5 bits (then here we take
			#   the mirror with XOR)
			# TODO: this doesn't seem to actually be saving this
			#         new .enc value into self.cubes[<neighbor pos>].enc
			#neighbor.enc -= (1 << ((5-dir) ^ 1))
			self.cubes[neighbor.pos].enc -= (1 << ((5-dir) ^ 1))
			#if neighbor.enc < 0:
			#	pass
		del self.cubes[pos]
		self.n -= 1
		self.canonical_info = None

	#def add_temporary(self, *, pos):
	#	# TODO: what are the type(s) of self.canonical_info? and is .copy() needed?
	#	canonical_info = self.canonical_info.copy()
	#	self.add(pos=pos)

	# for each cube, find its maximum value after a would-be rotation,
	#   and return the sorted list of those values
	def find_maximum_cube_values(self):
		global maximum_rotated_cube_values
		return sorted([maximum_rotated_cube_values[cube.enc] for cube in self.cubes.values()])
		#max_cube_rotated_value = 0
		#for cube in self.cubes.values():
		#	max_cube_rotated_value = max(maximum_rotated_cube_values[cube.enc], max_cube_rotated_value)
		#return max_cube_rotated_value

	# for the given start cube and rotation, find the encoding of the polycube
	def make_encoding_orig(self, *, start_cube, rotations_index):
		global rotations
		rotation = rotations[rotations_index]
		# cubes are added to this list in the same order as specified by the rotation
		encoded_cubes = [start_cube]
		self.temp += 1
		start_cube.tmp = self.temp
		i = 0
		while len(encoded_cubes) < self.n:
			for j in rotation:
				neighbor = encoded_cubes[i].neighbors[j]
				if neighbor is None or neighbor.temp == self.temp:
					continue
				neighbor.tmp = self.temp
				encoded_cubes.append(neighbor)
			i += 1
		# create single encoded int using only the first i cubes
		#   (since cubes are not explicitly in the encoding list
		#   if they are fully specified by a previous cube)
		encoding = self.ordered_cubes_to_int(ordered_cubes=encoded_cubes[0:i], rotations_index=rotations_index)
		# return the encoding and the position of the last cube
		#   in the encoding order
		#return (encoding, encoded_cubes[self.n - 1].pos)
		return (encoding, encoded_cubes[-1].pos)

	# for the given start cube and rotation, find the encoding of the polycube
	def make_encoding_unfinished(self, *, start_cube, rotations_index):
		global rotations
		rotation = rotations[rotations_index]
		# cubes are added to this list in the same order as specified by the rotation
		encoded_cubes = [start_cube]
		included_cube_pos = set()
		included_cube_pos.add(start_cube.pos)
		encoding = rotationTable[start_cube.enc][rotations_index]
		# use cursor moving from left (most significant bit) to right (least significant bit)
		cursor = 5
		cursor_cube = start_cube
		while len(included_cube_pos) < self.n:
			# by the time the cursor reaches the far right (least significant bit)
			#   of the encoding, we should already have all cubes represented
			#   in the encoding, so this if check should never happen
			if cursor < 0:
				pass
			# advance the cursor if the encoding has a 0 bit at the cursor
			if encoding & (1 << cursor) == 0:
				cursor -= 1
				continue
			# the 5th bit from the left corresponds to the 0th item in the rotation array
			rot_dir = rotation[cursor - 5]
			neighbor = cursor_cube.neighbors[rot_dir]
			# since the cube's enc has a 1 bit in the rotated direction,
			#   there should be a neighbor there!
			if neighbor is None:
				pass
			included_cube_pos.add(neighbor.pos)
			# if the neighbor itself has neighbor(s)
			cursor -= 1

	def truncate_redundant_cubes(self, *, ordered_cubes, rotation):
		# for now, don't truncate anything
		return ordered_cubes

	def make_encoding_recursive(self, *, start_cube_pos, rotation, included_cube_pos):
		#if depth > 50:
		#	pass
		ordered_cubes = [self.cubes[start_cube_pos]]
		included_cube_pos.add(start_cube_pos)
		#if not start_cube.pos in included_cube_pos:
			#ordered_cubes.append(start_cube)
			#included_cube_pos.add(start_cube.pos)
		for direction in rotation:
			#if len(included_cube_pos) == self.n:
			#	break
			neighbor = self.cubes[start_cube_pos].neighbors[direction]
			if neighbor is None or neighbor.pos in included_cube_pos:
				continue
			ordered_cubes += self.make_encoding_recursive(start_cube_pos=neighbor.pos, rotation=rotation, included_cube_pos=included_cube_pos)
		return ordered_cubes

	def make_encoding(self, *, start_cube_pos, rotations_index):
		global rotations
		# uses a recursive depth-first encoding of all cubes, using
		#   the provided rotation's order to traverse the cubes
		# TODO: return only an "ordered cubes" list, and include all
		#         cubes in it
		ordered_cubes = self.make_encoding_recursive( \
			start_cube_pos=start_cube_pos, \
			rotation=rotations[rotations_index], \
			included_cube_pos=set() \
		)
		# TODO: going in rotation-order, create the int encoding
		#         and stop as soon as we've processed enough cubes
		#         to have at least one '1' bit for each cube in
		#         the polycube
		encoding_cubes = self.truncate_redundant_cubes(ordered_cubes=ordered_cubes, rotation=rotations[rotations_index])
		encoding = self.ordered_cubes_to_int(ordered_cubes=encoding_cubes, rotations_index=rotations_index)
		# return the encoding and the position of the last cube
		#   in the encoding order
		#return (encoding, encoded_cubes[self.n - 1].pos)
		#return (encoding, encoded_cubes[-1].pos)
		encoding_and_last_pos = (encoding, ordered_cubes[-1].pos)
		#print(f'for {self.n=}, encoding: {int_to_bit_list_unbound(encoding_and_last_pos[0])}')
		return encoding_and_last_pos

	def ordered_cubes_to_int(self, *, ordered_cubes, rotations_index):
		global rotation_table
		encoding = 0
		for i,cube in enumerate(ordered_cubes):
			encoding = encoding << 6
			#encoding += (rotation_table[cube.enc][rotations_index] << (6 * i))
			encoding += rotation_table[cube.enc][rotations_index]
		return encoding

	def are_canonical_infos_equal(self, a, b):
		# we can just compare the encoded int values, so we don't
		#   need to compare the rest of the canonical info
		#return a[0] == b[0] and a[1] == b[1] and a[2] == b[2]
		return a[0] == b[0]

	def find_canonical_info(self):
		global maximum_rotated_cube_values
		if self.canonical_info is not None:
			return self.canonical_info

		maximum_rotated_values_of_cubes = self.find_maximum_cube_values()
		max_rotated_value_of_any_cube = maximum_rotated_values_of_cubes[-1]
		canonical = [0, set(), maximum_rotated_values_of_cubes]
		encoding_diff = 0
		for cube in self.cubes.values():
			# there could be more than one cube with the maximum rotated value
			if maximum_rotated_cube_values[cube.enc] == max_rotated_value_of_any_cube:
				# use all rotations that give this cube its maximum value
				for rotations_index in maximum_cube_rotation_indices[cube.enc]:
					encoded_polycube = self.make_encoding(start_cube_pos=cube.pos, rotations_index=rotations_index)
					encoding_diff = encoded_polycube[0] - canonical[0]
					if encoding_diff > 0:
						canonical[0] = encoded_polycube[0]
						canonical[1].clear()
						canonical[1].add(encoded_polycube[1])
					elif encoding_diff == 0:
						canonical[1].add(encoded_polycube[1])
		self.canonical_info = canonical
		return canonical

	def extend(self, *, limit_n):
		global direction_costs
		global n_counts
		# since this is a valid polycube, increment the count
		n_counts[self.n] += 1

		#if False and self.n == 4: # debug printing
		#	print(f"counting new polycube w {self.n=}")
		#	for cube in self.cubes.values():
		#		print(f"  {cube.coords=}")

		# we are done if we've reached the desired n,
		#   which we need to stop at because we are doing
		#   a depth-first recursive evaluation
		if self.n == limit_n:
			return
		# keep a Set of all evaluated positions so we don't repeat them
		#   TODO: can we initialize this with the contents of self.cubes
		#           and skip the "or try_pos in self.cubes" check below?
		#tried_pos = set()
		tried_pos = set(self.cubes.keys())

		tried_canonicals = []

		canonical_orig = self.find_canonical_info()

		# faster to declare a variable here, ahead of the loop?
		#   or can the varaible just be declared and used inside the loop?
		try_pos = 0
		# for each cube, for each direction, add a cube
		for cube in self.cubes.values():
			for direction_cost in direction_costs:
				try_pos = cube.pos + direction_cost
				#if try_pos in tried_pos or try_pos in self.cubes:
				if try_pos in tried_pos:
					continue
				tried_pos.add(try_pos)

				# create p+1
				tmp_add = self.copy()
				tmp_add.add(pos=try_pos)

				#if False: # debug print coords of all cubes
				#	tmp_add.cubes[try_pos].coords = self.cubes[cube.pos].coords.copy()
				#	if direction_cost == -1:
				#		tmp_add.cubes[try_pos].coords['x'] -= 1
				#	elif direction_cost == 1:
				#		tmp_add.cubes[try_pos].coords['x'] += 1
				#	elif direction_cost == -100:
				#		tmp_add.cubes[try_pos].coords['y'] -= 1
				#	elif direction_cost == 100:
				#		tmp_add.cubes[try_pos].coords['y'] += 1
				#	elif direction_cost == -10_000:
				#		tmp_add.cubes[try_pos].coords['z'] -= 1
				#	elif direction_cost == 10_000:
				#		tmp_add.cubes[try_pos].coords['z'] += 1
				#	else:
				#		print("what?")
				#		sys.exit(1)
				#	if any([abs(cube.coords['x']) == 1 and abs(cube.coords['y']) == 1 and cube.coords['z'] == 0 for cube in tmp_add.cubes.values()]):
				#		pass

				# skip if we've already seen some p+1 with the same canonical representation
				#   (comparing the bitwise int only)
				canonical_try = tmp_add.find_canonical_info()
				if any(canonical_try[0] == tried_canonical[0] for tried_canonical in tried_canonicals):
					continue

				tried_canonicals.append(canonical_try)
				# why are we doing this?
				if try_pos in canonical_try[2]:
					print("we are doing the thing")
					tmp_add.extend(limit_n=limit_n)
					continue

				# remove the last of the ordered cubes in p+1
				tmp_remove = tmp_add.copy()
				#if tmp_remove.n == 4:
				#	pass
				# enumerate the set of "last cubes", and grab one, where
				#   enumerate.__next__() returns a tuple of (index, value)
				#   and thus we need to use the 1th element of the tuple
				tmp_remove.remove(pos=enumerate(canonical_try[1]).__next__()[1])
				# if p+1-1 has the same canonical representation as p, count it as a new unique polycube
				#   and continue recursion into that p+1
				canonical_try_removed = tmp_remove.find_canonical_info()
				if self.are_canonical_infos_equal(canonical_try_removed, canonical_orig):
					tmp_add.extend(limit_n=limit_n)
				#else:
				#	# just here for debugging stepping purposes
				#	pass

last_interrupt_time = 0

def interrupt_handler(sig, frame):
	global last_interrupt_time
	now = time.time()
	do_halt = False
	if now - last_interrupt_time < 2:
		do_halt = True
	else:
		last_interrupt_time = now
		#print('\nresults so far:')
		print_results()
	if do_halt:
		print('\nstopping...')
		sys.exit(0)

def print_results():
	global n_counts
	print('\n\nresults:')
	for n,v in enumerate(n_counts):
		if n > 0 and v > 0:
			print(f'n = {n:>2}: {v}')

if __name__ == "__main__":

	# test placing three cubes in a row, and see if the middle cube is
	#   erroneously listed as the "last" cube in the encoding
	#a = Polycube(create_initial_cube=True)
	#a.add(pos=1)
	#a.add(pos=-1)
	#canonical_a = a.find_canonical_info()

	# test that different 3-cube "L" shapes have the same canonical encoding
	b = Polycube(create_initial_cube=False)
	b.add(pos=0)
	b.add(pos=-1)
	b.add(pos=-100)
	b_enc = b.find_canonical_info()
	c = Polycube(create_initial_cube=False)
	c.add(pos=-1)
	c.add(pos=-100)
	c.add(pos=-101)
	c_enc = c.find_canonical_info()
	d = Polycube(create_initial_cube=False)
	d.add(pos=1)
	d.add(pos=100)
	d.add(pos=101)
	d_enc = d.find_canonical_info()
	e = Polycube(create_initial_cube=False)
	e.add(pos=0)
	e.add(pos=100)
	e.add(pos=101)
	e_enc = e.find_canonical_info()
	if b_enc[0] == c_enc[0] and b_enc[0] == d_enc[0] and b_enc[0] == e_enc[0]:
		pass
	else:
		print("two different 3-cube 'L' shapes should have the same canonical encoding", file=sys.stderr)
		sys.exit(1)

	signal.signal(signal.SIGINT, interrupt_handler)
	print("use Ctrl+C once to print current results, or twice to stop")
	start_time = time.perf_counter()
	# enumerate all valid polycubes of size n
	p = Polycube(create_initial_cube=True)
	# i guess we'll use recursion for this first attempt at this:
	# - we'll extend each "minimal" polycube found at each n level
	# - this avoid having to save anything in memory
	# - a drawback is that each "minimal" polycube to count must
	#     do a list lookup to increment the counter
	p.extend(limit_n=4 if len(sys.argv) < 2 else int(sys.argv[1]))
	print_results()
	#for i in range(100):
	#	time.sleep(1)
	#	print(f"boop {i}")
	print(f'elapsed seconds: {time.perf_counter() - start_time}');