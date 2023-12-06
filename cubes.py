# python3
#

import argparse
import concurrent.futures
from copy import deepcopy
from datetime import timedelta
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
	for i,rotation in enumerate(rotations):
		if rotate_value(cube_enc, rotation) == max_value:
			maximum_cube_rotation_indices[cube_enc].append(i)

# the n value at which found canonical polycubes are
#   submitted as jobs for threads to continue recursion
# at n=4, there are only 8 possible polycubes, so only
#   8 threads could ever run simultaneously starting at
#   n=4
# splitting at n=5 or n=6 seems fine for a single machine
#   but if splitting over many machines with many cores
#   available a higher n value will allow more cores to
#   run simultaneously:
#   n | 4 |  5 |  6  |   7  |   8  |   9   |   10
#   # | 8 | 29 | 166 | 1023 | 6922 | 48311 | 346543
initial_delegator_spawn_n = 6

# count of unique polycubes of size n
n_counts = [0 for i in range(22)]

# global stuff accessible by threads
global_pool_executor = None
outstanding_threads = 0
last_count_increment_time = None
completed_threads = 0

class Cube:

	def __init__(self, *, pos):
		# position in the polycube defined as (x + 100y + 10,000z)
		self.pos = pos
		# references to neighbor Cube instances
		self.neighbors = [None, None, None, None, None, None]
		# integer encoding for this cube's neighbors:
		# 6 bits per cube, where a 1 represents a present neighbor in that direction
		self.enc = 0

	def copy(self):
		new_cube = Cube(pos=self.pos)
		new_cube.enc = self.enc
		new_cube.neighbors = self.neighbors.copy()
		return new_cube

def extend_with_thread_pool_callback(future):
	global n_counts
	global last_count_increment_time
	global outstanding_threads
	global completed_threads
	for n,count in enumerate(future.result()):
		n_counts[n] += count
	last_count_increment_time = time.perf_counter()
	# decrement the number of submitted/running threads
	outstanding_threads -= 1
	completed_threads += 1

# the function each thread will run
def extend_with_thread_pool(*, polycube, limit_n, initial_delegator):
	global direction_costs
	global global_pool_executor
	global outstanding_threads
	global initial_delegator_spawn_n

	# we are done if we've reached the desired n,
	#   which we need to stop at because we are doing
	#   a depth-first recursive evaluation
	if polycube.n == limit_n:
		return []

	found_counts_by_n = [0 for i in range(22)]

	# keep a Set of all evaluated positions so we don't repeat them
	tried_pos = set(polycube.cubes.keys())

	tried_canonicals = set()
	canonical_orig = polycube.find_canonical_info()
	tmp_add = polycube.copy()

	# faster to declare a variable here, ahead of the loop?
	#   or can the varaible just be declared and used inside the loop?
	try_pos = 0

	# for each cube, for each direction, add a cube
	for cube_pos in polycube.cubes:
		for direction_cost in direction_costs:
			try_pos = cube_pos + direction_cost
			if try_pos in tried_pos:
				continue
			tried_pos.add(try_pos)

			# create p+1
			tmp_add.add(pos=try_pos)

			# skip if we've already seen some p+1 with the same canonical representation
			#   (comparing the bitwise int only)
			canonical_try = tmp_add.find_canonical_info()
			if canonical_try[0] in tried_canonicals:
				tmp_add.remove(pos=try_pos)
				continue

			tried_canonicals.add(canonical_try[0])
			# why are we doing this?
			# this seems to never run, so commenting this out for now
			#if try_pos in canonical_try[2]:
			#	print("we are doing the thing")
			#	tmp_add.copy().extend_single_thread(limit_n=limit_n)
			#	# revert creating p+1 to try adding a cube at another position
			#	tmp_add.remove(pos=try_pos)
			#	continue

			# remove the last of the ordered cubes in p+1
			least_significant_cube_pos = enumerate(canonical_try[1]).__next__()[1]

			# enumerate the set of "last cubes", and grab one, where
			#   enumerate.__next__() returns a tuple of (index, value)
			#   and thus we need to use the 1th element of the tuple
			tmp_add.remove(pos=least_significant_cube_pos)

			# if p+1-1 has the same canonical representation as p, count it as a new unique polycube
			#   and continue recursion into that p+1
			if tmp_add.find_canonical_info()[0] == canonical_orig[0]:
				# replace the least significant cube we just removed
				tmp_add.add(pos=least_significant_cube_pos)
				# allow the found polycube to be counted elsewhere
				found_counts_by_n[tmp_add.n] += 1
				# the initial delegator submits jobs for threads,
				#   but only if the found polycube has n=6
				if initial_delegator and tmp_add.n == initial_delegator_spawn_n:
					# increment the number of submitted+running threads
					outstanding_threads += 1
					submitted_future = global_pool_executor.submit(extend_with_thread_pool, polycube=tmp_add.copy(), limit_n=limit_n, initial_delegator=False)
					submitted_future.add_done_callback(extend_with_thread_pool_callback)

				# otherwise, continue recursion within this thread
				else:
					further_counts = extend_with_thread_pool(polycube=tmp_add.copy(), limit_n=limit_n, initial_delegator=initial_delegator)
					for n,count in enumerate(further_counts):
						found_counts_by_n[n] += count

			# undo the temporary removal of the least significant cube,
			#   but only if it's not the same as the cube we just tried
			#   since we remove that one before going to the next iteration
			#   of the loop
			elif least_significant_cube_pos != try_pos:
				tmp_add.add(pos=least_significant_cube_pos)

			# revert creating p+1 to try adding a cube at another position
			tmp_add.remove(pos=try_pos)

	if initial_delegator and polycube.n == 1:
		print(f"initial delegator is done, {outstanding_threads=}")
	return found_counts_by_n

class Polycube:

	# initialize with 1 cube at (0, 0, 0)
	def __init__(self, create_initial_cube):
		self.canonical_info = None
		# number of cubes in this polycube
		self.n = 0
		# positions of cubes in this polycube
		self.cubes = {}
		if create_initial_cube:
			self.n = 1
			self.cubes[0] = Cube(pos=0)

	def copy(self):
		new_polycube = Polycube(create_initial_cube=False)
		new_polycube.n = self.n
		new_polycube.cubes = {}
		for pos,cube in self.cubes.items():
			new_polycube.cubes[pos] = cube.copy()
		# thanks to https://stackoverflow.com/a/15214597/259456
		new_polycube.canonical_info = deepcopy(self.canonical_info)
		return new_polycube

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
			# use XOR to flip between each direction and its opposite
			#   to set the neighbor's neighbor to the new cube
			#   (0<->1, 2<->3, 4<->5)
			dir_cube.neighbors[direction ^ 1] = new_cube
			# we use rotation of [0,1,2,3,4,5] where the '0'
			#   direction is -x and is the most significant bit
			#   in each cube's .enc value, so we need '0' to
			#   cause a left shift by 5 bits (and here we use
			#   XOR to flip to the opposite direction)
			# try bitwise OR instead of adding (but the original javascript
			#   implementation used addition here)
			#dir_cube.enc += (1 << ((5-direction) ^ 1))
			dir_cube.enc |= (1 << ((5-direction) ^ 1))

	def remove(self, *, pos):
		# remove this cube from each of its neighbors
		for dir,neighbor in enumerate(self.cubes[pos].neighbors):
			if neighbor is None:
				continue
			neighbor_enc_orig = neighbor.enc
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
		del self.cubes[pos]
		self.n -= 1
		self.canonical_info = None

	# for each cube, find its maximum value after a would-be rotation,
	#   and return the sorted list of those values
	def find_maximum_cube_values(self):
		global maximum_rotated_cube_values
		return sorted([maximum_rotated_cube_values[cube.enc] for cube in self.cubes.values()])

	def truncate_redundant_cubes(self, *, ordered_cubes, rotation):
		# for now, don't truncate anything
		return ordered_cubes

	def make_encoding_recursive(self, *, start_cube_pos, rotation, included_cube_pos):
		ordered_cubes = [self.cubes[start_cube_pos]]
		included_cube_pos.add(start_cube_pos)
		for direction in rotation:
			neighbor = self.cubes[start_cube_pos].neighbors[direction]
			if neighbor is None or neighbor.pos in included_cube_pos:
				continue
			ordered_cubes += self.make_encoding_recursive(start_cube_pos=neighbor.pos, rotation=rotation, included_cube_pos=included_cube_pos)
		return ordered_cubes

	def make_encoding(self, *, start_cube_pos, rotations_index):
		global rotations
		# uses a recursive depth-first encoding of all cubes, using
		#   the provided rotation's order to traverse the cubes
		ordered_cubes = self.make_encoding_recursive( \
			start_cube_pos=start_cube_pos, \
			rotation=rotations[rotations_index], \
			included_cube_pos=set() \
		)
		# TODO: going in rotation-order, create the int encoding
		#         and stop as soon as we've processed enough cubes
		#         to have at least one '1' bit for each cube in
		#         the polycube (but this doesn't actually seem
		#         necesary... may be faster to just not do this)
		encoding_cubes = self.truncate_redundant_cubes(ordered_cubes=ordered_cubes, rotation=rotations[rotations_index])
		encoding = self.ordered_cubes_to_int(ordered_cubes=encoding_cubes, rotations_index=rotations_index)
		# return the encoding and the position of the last cube
		#   in the encoding order
		#encoding_and_last_pos = (encoding, ordered_cubes[-1].pos)
		#print(f'for {self.n=}, encoding: {int_to_bit_list_unbound(encoding_and_last_pos[0])}')
		return (encoding, ordered_cubes[-1].pos)

	def ordered_cubes_to_int(self, *, ordered_cubes, rotations_index):
		global rotation_table
		encoding = 0
		for i,cube in enumerate(ordered_cubes):
			encoding = encoding << 6
			encoding += rotation_table[cube.enc][rotations_index]
		return encoding

	def are_canonical_infos_equal(self, a, b):
		# we can just compare the encoded int values, so we don't
		#   need to compare the rest of the canonical info
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

	def extend_single_thread(self, *, limit_n):
		global direction_costs
		global n_counts
		# since this is a valid polycube, increment the count
		n_counts[self.n] += 1

		# we are done if we've reached the desired n,
		#   which we need to stop at because we are doing
		#   a depth-first recursive evaluation
		if self.n == limit_n:
			return
		# keep a Set of all evaluated positions so we don't repeat them
		tried_pos = set(self.cubes.keys())

		tried_canonicals = set()

		canonical_orig = self.find_canonical_info()

		tmp_add = self.copy()

		# faster to declare a variable here, ahead of the loop?
		#   or can the varaible just be declared and used inside the loop?
		try_pos = 0
		# for each cube, for each direction, add a cube
		for cube_pos in self.cubes:
			for direction_cost in direction_costs:
				try_pos = cube_pos + direction_cost
				if try_pos in tried_pos:
					continue
				tried_pos.add(try_pos)

				# create p+1
				tmp_add.add(pos=try_pos)

				# skip if we've already seen some p+1 with the same canonical representation
				#   (comparing the bitwise int only)
				canonical_try = tmp_add.find_canonical_info()
				if canonical_try[0] in tried_canonicals:
					tmp_add.remove(pos=try_pos)
					continue

				tried_canonicals.add(canonical_try[0])
				# why are we doing this?
				# this seems to never run, so commenting this out for now
				#if try_pos in canonical_try[2]:
				#	print("we are doing the thing")
				#	tmp_add.copy().extend_single_thread(limit_n=limit_n)
				#	# revert creating p+1 to try adding a cube at another position
				#	tmp_add.remove(pos=try_pos)
				#	continue

				# remove the last of the ordered cubes in p+1
				least_significant_cube_pos = enumerate(canonical_try[1]).__next__()[1]

				# enumerate the set of "last cubes", and grab one, where
				#   enumerate.__next__() returns a tuple of (index, value)
				#   and thus we need to use the 1th element of the tuple
				tmp_add.remove(pos=least_significant_cube_pos)

				# if p+1-1 has the same canonical representation as p, count it as a new unique polycube
				#   and continue recursion into that p+1
				if tmp_add.find_canonical_info()[0] == canonical_orig[0]:
					# replace the least significant cube we just removed
					tmp_add.add(pos=least_significant_cube_pos)
					# make a copy here for continuing recursion upon
					tmp_add.copy().extend_single_thread(limit_n=limit_n)

				# undo the temporary removal of the least significant cube,
				#   but only if it's not the same as the cube we just tried
				#   since we remove that one before going to the next iteration
				#   of the loop
				elif least_significant_cube_pos != try_pos:
					tmp_add.add(pos=least_significant_cube_pos)

				# revert creating p+1 to try adding a cube at another position
				tmp_add.remove(pos=try_pos)

	def extend(self, *, limit_n):
		global global_pool_executor
		global n_counts
		# use the concurrent version of this function if we have a pool_executor
		if global_pool_executor is None:
			self.extend_single_thread(limit_n=limit_n)
		else:
			# track that we're about to start/queue a thread
			counts = extend_with_thread_pool(polycube=self, limit_n=limit_n, initial_delegator=True)
			for n,count in enumerate(counts):
				n_counts[n] += count


def print_results():
	global n_counts
	print(f'\n\nresults:')
	for n,v in enumerate(n_counts):
		if n > 0 and v > 0:
			print(f'n = {n:>2}: {v}')

if __name__ == "__main__":
	arg_parser = argparse.ArgumentParser(\
		description='Count the number of (rotationally) unique polycubes containing up to n cubes')
	arg_parser.add_argument('n', metavar='<n>', type=int,
		help='the number of cubes the largest counted polycube should contain')
	arg_parser.add_argument('--threads', metavar='<threads>', type=int, required=False,
		default=0, help='0 for single-threaded, or the maximum number of threads to spawn simultaneously (default=0)')
	arg_parser.add_argument('--spawn-n', metavar='<spawn-n>', type=int, required=False,
		default=7, help='the smallest polycubes for which each will spawn a thread, higher->more shorter-lived threads (default=7)')

	args = arg_parser.parse_args()
	if args.n < 0 or args.threads < 0 or args.spawn_n < 0:
		arg_parser.print_help()
		sys.exit(1)

	initial_delegator_spawn_n = args.spawn_n

	start_time = time.perf_counter()
	start_eta_time = time.time()
	if args.threads == 0:
		p = Polycube(create_initial_cube=True)
		# enumerate all valid polycubes up to size limit_n
		p.extend(limit_n=args.n)
	else:
		with concurrent.futures.ProcessPoolExecutor(max_workers=args.threads) as pool_executor:
			global_pool_executor = pool_executor
			p = Polycube(create_initial_cube=True)
			n_counts[1] += 1
			# enumerate all valid polycubes up to size limit_n
			p.extend(limit_n=args.n)
			# we have to busy wait here, inside this "with ... as pool_executor"
			#   block, in order to keep the ThreadPoolExecutor alive
			# while waiting, we can do useful things here like showing
			#   useful timing/counts data
			time.sleep(2.0)
			while outstanding_threads > 0:
				time_elapsed = time.time() - start_eta_time
				seconds_per_thread = time_elapsed / completed_threads
				seconds_remaining = timedelta(seconds=round(seconds_per_thread * outstanding_threads))
				pct_complete = (float(completed_threads) * 100.0) / (float(completed_threads) + float(outstanding_threads))
				total_seconds = round((seconds_per_thread * outstanding_threads) + time_elapsed)
				total_seconds_remaining = timedelta(seconds=round(total_seconds))
				print(f'    {round(pct_complete,4)}% complete, ETA: [{seconds_remaining}], total: [{total_seconds_remaining}], counting for n={args.n}: [{n_counts[args.n]}], outstanding threads: [{outstanding_threads}]       ', end='\r')
				time.sleep(1.0)
			print("outstanding_threads is empty")
	print_results()
	if last_count_increment_time is None:
		last_count_increment_time = time.perf_counter()
	print(f'elapsed seconds: {last_count_increment_time - start_time}')
