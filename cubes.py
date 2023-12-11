# python3
#

import argparse
from copy import deepcopy
from datetime import datetime, timedelta
import gzip
import json
from multiprocessing import Manager, Pipe, Process
from pathlib import Path
from queue import Empty as QueueEmpty
import random
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
	for i,rotation in enumerate(rotations):
		if rotate_value(cube_enc, rotation) == max_value:
			maximum_cube_rotation_indices[cube_enc].append(i)

# from https://oeis.org/A000162
# these are the number of unique polycubes of size n,
#   which is kind of funny to put in a program that
#   calculates these values -- but these are needed to
#   help calculate estimated time remaining
well_known_n_counts = [0, 1, 1, 2, 8, 29, 166, 1023, 6922, 48311, 346543, 2522522, 18598427, 138462649, 1039496297, 7859514470, 59795121480]

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
initial_delegator_spawn_n = 8

# count of unique polycubes of size n
n_counts = [0]*23

class AbandonEncoding(Exception):
    pass

class HaltSignal(Exception):
	pass

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

# the initial delegator worker begins here
def delegate_extend(polycube, n, halt_pipe_recv, submit_queue, response_queue, spawn_n):
	try:
		extend_with_thread_pool(
			polycube=polycube,
			limit_n=n,
			delegate_at_n=spawn_n,
			submit_queue=submit_queue,
			response_queue=response_queue,
			halt_pipe=halt_pipe_recv,
			depth=0)
	except HaltSignal:
		# maybe indicate that the initial delegator worker was
		#   halted with a special-case None value here
		response_queue.put((False, None))

# the regular workers begin here
def worker_extend_outer(n, halt_pipe_recv, done_pipe_recv, submit_queue, response_queue):
	halted = False
	# wait for work to arrive if halt hasn't been signalled yet
	#while not halted and not submit_queue.empty():
	while not halted:
		try:
			polycube = submit_queue.get(block = True, timeout = 1.0)
			extend_with_thread_pool(
				polycube=polycube,
				limit_n=n,
				delegate_at_n=0,
				submit_queue=submit_queue,
				response_queue=response_queue,
				halt_pipe=halt_pipe_recv,
				depth=0)
		except QueueEmpty:
			if not halted and (halt_pipe_recv.poll() or done_pipe_recv.poll()):
				halted = True
		except HaltSignal:
			# we need to record the intitial polycube as unevaluated
			#   (to be resumed later)
			response_queue.put((False, polycube.copy()))
			halted = True

	# after halt, drain the queue
	found_something = True
	while found_something:
		try:
			polycube = submit_queue.get(block = True, timeout = 1.0)
			# put a message here to indicate that this Polycube is
			#   unevaluated
			response_queue.put((False, polycube))
		except QueueEmpty:
			found_something = False

def extend_with_thread_pool(*, polycube, limit_n, delegate_at_n, submit_queue, response_queue, halt_pipe, depth):
	global direction_costs

	# we are done if we've reached the desired n,
	#   which we need to stop at because we are doing
	#   a depth-first recursive evaluation
	if polycube.n == limit_n:
		return []

	found_counts_by_n = [0]*23

	# keep a Set of all evaluated positions so we don't repeat them
	tried_pos = set(polycube.cubes.keys())

	tried_canonicals = set()
	canonical_orig = polycube.find_canonical_info()
	tmp_add = polycube.copy()

	# faster to declare a variable here, ahead of the loop?
	#   or can the varaible just be declared and used inside the loop?
	try_pos = 0

	# if halt has been signalled, abandon the evaluation
	#   of this polycube
	# since this function is run many many times by each process/thread,
	#   we can greatly reduce use of Pipe.poll() and increase per-
	#   process CPU utilization from ~90% to ~98% (at least on my
	#   machine) by only checking for halt every 1000th iteration
	if random.randrange(0, 1000) == 0 and halt_pipe.poll():
		raise HaltSignal

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
				#   but only if the found polycube has n=spawn_n
				if delegate_at_n > 0 and tmp_add.n == delegate_at_n:
					submit_queue.put(tmp_add.copy())

				# otherwise, continue recursion within this thread
				else:
					#further_counts = extend_with_thread_pool(polycube=tmp_add.copy(), limit_n=limit_n, initial_delegator=initial_delegator, write_to_file_queue=write_to_file_queue, halt_pipe=halt_pipe, submitted_threads=submitted_threads)
					further_counts = extend_with_thread_pool(\
						polycube=tmp_add.copy(), \
						limit_n=limit_n, \
						delegate_at_n=delegate_at_n, \
						submit_queue=submit_queue, \
						response_queue=response_queue, \
						halt_pipe=halt_pipe, \
						depth=depth+1)
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

	if depth == 0:
		response_queue.put((True, found_counts_by_n))
		return
	else:
		return found_counts_by_n

def extend_single_thread(*, polycube, limit_n):
	global direction_costs
	global n_counts

	# since this is a valid polycube, increment the count
	n_counts[polycube.n] += 1

	# we are done if we've reached the desired n,
	#   which we need to stop at because we are doing
	#   a depth-first recursive evaluation
	if polycube.n == limit_n:
		return
	# keep a Set of all evaluated positions so we don't repeat them
	tried_pos = set(polycube.cubes.keys())

	tried_canonicals = set()

	canonical_orig = polycube.find_canonical_info()

	# faster to declare a variable here, ahead of the loop?
	#   or can the varaible just be declared and used inside the loop?
	try_pos = 0
	# for each cube, for each direction, add a cube
	# create a list to iterate over because the dict will change
	#   during recursion within the loop
	for cube_pos in list(polycube.cubes.keys()):
		for direction_cost in direction_costs:
			try_pos = cube_pos + direction_cost
			if try_pos in tried_pos:
				continue
			tried_pos.add(try_pos)

			# create p+1
			polycube.add(pos=try_pos)

			# skip if we've already seen some p+1 with the same canonical representation
			#   (comparing the bitwise int only)
			canonical_try = polycube.find_canonical_info()
			if canonical_try[0] in tried_canonicals:
				polycube.remove(pos=try_pos)
				continue

			tried_canonicals.add(canonical_try[0])
			# why are we doing this?
			# this seems to never run, so commenting this out for now
			#if try_pos in canonical_try[2]:
			#	print("we are doing the thing")
			#	extend_single_thread(polycube=polycube, limit_n=limit_n, depth=depth+1)
			#	# revert creating p+1 to try adding a cube at another position
			#	polycube.remove(pos=try_pos)
			#	continue

			# remove the last of the ordered cubes in p+1
			least_significant_cube_pos = enumerate(canonical_try[1]).__next__()[1]

			# enumerate the set of "last cubes", and grab one, where
			#   enumerate.__next__() returns a tuple of (index, value)
			#   and thus we need to use the 1th element of the tuple
			polycube.remove(pos=least_significant_cube_pos)

			# if p+1-1 has the same canonical representation as p, count it as a new unique polycube
			#   and continue recursion into that p+1
			if polycube.find_canonical_info()[0] == canonical_orig[0]:
				# replace the least significant cube we just removed
				polycube.add(pos=least_significant_cube_pos)
				extend_single_thread(polycube=polycube, limit_n=limit_n)

			# undo the temporary removal of the least significant cube,
			#   but only if it's not the same as the cube we just tried
			#   since we remove that one before going to the next iteration
			#   of the loop
			elif least_significant_cube_pos != try_pos:
				polycube.add(pos=least_significant_cube_pos)

			# revert creating p+1 to try adding a cube at another position
			polycube.remove(pos=try_pos)

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

	def make_encoding_recursive(self, *, start_cube_pos, rotation, included_cube_pos, best_encoding, rotations_index, offset, encoding):
		global rotation_table
		encoding = (encoding << 6) + rotation_table[self.cubes[start_cube_pos].enc][rotations_index]
		#print(f"{self.n=},{offset=}: new enc {int_to_bit_list_unbound(encoding)}")
		if encoding < (best_encoding >> (offset * 6)):
			#print(f"{self.n=},{offset=}:\n    best enc {int_to_bit_list_unbound(best_encoding >> (offset * 6))} >\n     new enc {int_to_bit_list_unbound(encoding)}")
			raise AbandonEncoding
		ordered_cubes = [self.cubes[start_cube_pos]]
		included_cube_pos.add(start_cube_pos)
		for direction in rotation:
			neighbor = self.cubes[start_cube_pos].neighbors[direction]
			if neighbor is None or neighbor.pos in included_cube_pos:
				continue
			cubes, encoding, offset = self.make_encoding_recursive(
				start_cube_pos=neighbor.pos,
				rotation=rotation,
				included_cube_pos=included_cube_pos,
				best_encoding=best_encoding,
				rotations_index=rotations_index,
				offset=offset-1,
				encoding=encoding
			)
			ordered_cubes += cubes
		return (ordered_cubes, encoding, offset)

	def make_encoding(self, *, start_cube_pos, rotations_index, best_encoding):
		global rotations
		# uses a recursive depth-first encoding of all cubes, using
		#   the provided rotation's order to traverse the cubes
		ordered_cubes, encoding, _ = self.make_encoding_recursive(
			start_cube_pos=start_cube_pos,
			rotation=rotations[rotations_index],
			included_cube_pos=set(),
			best_encoding=best_encoding,
			rotations_index=rotations_index,
			offset=self.n-1, # number of 6-bit shifts from the right, where the last cube has an offset of 0
			encoding=0
		)
		return (encoding, ordered_cubes[-1].pos)

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
		best_encoding = 0
		encoding_diff = 0
		for cube in self.cubes.values():
			# there could be more than one cube with the maximum rotated value
			if maximum_rotated_cube_values[cube.enc] == max_rotated_value_of_any_cube:
				# use all rotations that give this cube its maximum value
				for rotations_index in maximum_cube_rotation_indices[cube.enc]:
					try:
						encoded_polycube = self.make_encoding(start_cube_pos=cube.pos, rotations_index=rotations_index, best_encoding=best_encoding)
						encoding_diff = encoded_polycube[0] - canonical[0]
						if encoding_diff > 0:
							canonical[0] = encoded_polycube[0]
							canonical[1].clear()
							canonical[1].add(encoded_polycube[1])
							best_encoding = encoded_polycube[0]
						elif encoding_diff == 0:
							canonical[1].add(encoded_polycube[1])
					except AbandonEncoding:
						continue
		self.canonical_info = canonical
		return canonical


last_count_increment_time = None
last_interrupt_time = 0
do_halt = False

# trying to cleanly handle SIGINT (Ctrl+C) with the multiprocessing stuff
#   has been a nightmare (we'll still use it for single-thread)
# instead, we will periodically look for the presence of this file,
#   and, if found, will start the clean shutdown procedure
halt_file_path = Path(__file__).parent.joinpath("halt-signal.txt")
if halt_file_path.exists():
	print(f'found halt file [{halt_file_path}] already exists, stopping...')
	sys.exit(0)

def interrupt_handler(sig, frame):
	global last_interrupt_time
	now = time.time()
	if now - last_interrupt_time < 1:
		print('\nstopping...')
		print_results()
		# since this handler is only used for the single-threaded version,
		#   we can stop immediately
		sys.exit(0)
	else:
		last_interrupt_time = now
		print_results()

def print_results(is_complete=True):
	global n_counts
	print(f'\n\n{"" if is_complete else "partial "}results:')
	for n,v in enumerate(n_counts):
		if n > 0 and v > 0:
			print(f'n = {n:>2}: {v}')

def write_resume_file(n, spawn_n, polycubes, n_counts, total_elapsed_sec):
	json_file_timestamp = datetime.isoformat(datetime.now(),timespec='seconds').replace(':', '').replace('-', '')
	json_file = Path(__file__).parent.joinpath(f"halt-n{args.n}-{json_file_timestamp}.json.gz")
	print(f"writing {len(polycubes_to_write_to_disk)} polycubes to [{json_file}]...")
	json_content = {
		'n': n,
		'spawn_n': spawn_n,
		'counts': n_counts,
		'total_elapsed_sec': total_elapsed_sec,
		'unevaluated_polycubes': polycubes
	}
	# thanks to https://stackoverflow.com/a/49535758/259456
	#   for showing how to write gzip compressed content
	#   to a file
	# use 'xt' mode to open for writing text, failing if the file already exists
	#   (going with 'wt' mode for now, to overwrite the file because
	#   we may be saving hours of work and we don't want to just fail
	#   if a file with the same timestamp already exists)
	with gzip.open(json_file, 'wt', encoding="ascii") as f:
		json.dump(json_content, f)
	print("done writing to file")

def read_resume_file(resume_file_path):
	print(f"reading resume file [{resume_file_path}]...")
	json_content = None
	with gzip.open(resume_file_path, 'rt') as f:
		json_content = json.loads(f.read())
	print(f"done reading {len(json_content['unevaluated_polycubes'])} unevalutated polycubes from file")
	return (json_content['n'], json_content['spawn_n'], json_content['counts'], json_content['total_elapsed_sec'], json_content['unevaluated_polycubes'])

if __name__ == "__main__":

	resume_file_path = None
	polycubes_to_resume = []

	arg_parser = argparse.ArgumentParser(\
		description='Count the number of (rotationally) unique polycubes containing up to n cubes')
	arg_parser.add_argument('-n', metavar='<n>', type=int, required=False, default=-1,
		help='the number of cubes the largest counted polycube should contain (>1)')
	arg_parser.add_argument('--threads', metavar='<threads>', type=int, required=False, default=0,
		help='0 for single-threaded, or >1 for the maximum number of threads to spawn simultaneously (default=0)')
	arg_parser.add_argument('--spawn-n', metavar='<spawn-n>', type=int, required=False, default=8,
		help='the smallest polycubes for which each will spawn a thread, higher->more shorter-lived threads (default=7)')
	arg_parser.add_argument('--resume-from-file', metavar='<resume-file>', required=False,
		help='a .json.gz file previously created by this script')

	args = arg_parser.parse_args()
	if args.resume_from_file:
		if args.threads < 2:
			print("must use >1 thread if resuming from <resume_file>", file=sys.stderr)
			arg_parser.print_help()
			sys.exit(1)
		resume_file_path = Path(args.resume_from_file)
		if not resume_file_path.is_file():
			print(f'<resume-file> [{resume_file_path}] does not exist')
			sys.exit(1)
	elif args.n < 2 or args.threads < 0 or args.threads == 1 or args.spawn_n < 0:
		arg_parser.print_help()
		sys.exit(1)

	# load resume file content
	previous_total_elapsed_sec = 0.0
	if resume_file_path is not None:
		args.n, args.spawn_n, n_counts, previous_total_elapsed_sec, polycubes_to_resume = read_resume_file(resume_file_path)

	complete = False
	start_time = time.perf_counter()
	start_eta_time = time.time()
	polycube = Polycube(create_initial_cube=True)
	if args.threads == 0:
		print(f"use Ctrl+C once to print current results, or twice to stop")
		# for the single-threaded implementation, we can
		#   cleanly handle SIGINT (Ctrl+C)
		signal.signal(signal.SIGINT, interrupt_handler)
		# enumerate all valid polycubes up to size limit_n
		#   in a single thread
		extend_single_thread(polycube=polycube, limit_n=args.n)
	else:
		saved_worker_jobs = 0
		total_worker_jobs = well_known_n_counts[args.spawn_n]
		compl_worker_jobs = 0 if resume_file_path is None else total_worker_jobs - len(polycubes_to_resume)
		print(f"to halt early, create the file [{halt_file_path}]\n")
		polycubes_to_write_to_disk = []
		# the manager will provide queues and a pipe for communication
		#   with the child processes
		with Manager() as manager:
			# the child processes will return both counts for fully-
			#   evaluated Polycubes and also non-evaluated Polycubes
			#   to write to disk for continuing later
			response_queue = manager.Queue()
			# these are the found canonical Polycubes of whatever
			#   size that the child processes will evaluate
			submit_queue = manager.Queue()

			# the send and receive ends of the pipe for signalling
			#  that an early halt has been requested
			halt_pipe_recv, halt_pipe_send = Pipe(duplex=False)
			# the send and receive ends of the pipe for signalling
			#  to the workers to stop looking for jobs
			done_pipe_recv, done_pipe_send = Pipe(duplex=False)

			initial_workers_to_spawn = args.threads
			delegator_proc = None
			# create polycubes if resuming, and enqueue them
			if resume_file_path:
				while len(polycubes_to_resume) > 0:
					polycube = Polycube(create_initial_cube=False)
					# consume the list of polycube positions data since
					#   we don't need it to linger in memory after this
					for cube_pos in polycubes_to_resume.pop():
						polycube.add(pos=cube_pos)
					submit_queue.put(polycube)
				initial_workers_to_spawn = args.threads
			else:
				# initially spawn threads-1 worker threads, plus one
				#   thread for the initial work delegator
				initial_workers_to_spawn = args.threads - 1
				n_counts[1] = 1
				delegator_proc = Process(target=delegate_extend, args=(polycube, args.n, halt_pipe_recv, submit_queue, response_queue, args.spawn_n))
				delegator_proc.start()
			processes = []
			for i in range(0, initial_workers_to_spawn):
				p = Process(target=worker_extend_outer, args=(args.n, halt_pipe_recv, done_pipe_recv, submit_queue, response_queue))
				processes.append(p)
				p.start()
			halted = False
			last_stats_and_halt = time.time()
			while not halted or not submit_queue.empty() or not response_queue.empty():
				# once the initial work delegator has finished,
				#   spawn a new worker thread
				if not halted and resume_file_path is None and not delegator_proc.is_alive() and len(processes) < args.threads:
					print("\ninitial delegator thread has finished, spawning a new worker thread")
					# the initial delegator thread submits its results through
					#   the same response_queue as the rest of the workers, but it shouldn't
					#   be counted as a completed worker job
					compl_worker_jobs -= 1
					p = Process(target=worker_extend_outer, args=(args.n, halt_pipe_recv, done_pipe_recv, submit_queue, response_queue))
					processes.append(p)
					p.start()
				# check for halt file
				if not halted:
					if halt_file_path.is_file():
						print(f"\nfound halt file [{halt_file_path}], stopping...")
						# signal to the threads that they should stop
						halt_pipe_send.send(1)
						halted = True
				# check if everything has completed
				if not halted and (delegator_proc is None or not delegator_proc.is_alive()) and submit_queue.empty():
					print(f"\nlooks like we have finished!  stopping...")
					# signal to the threads that they should stop
					done_pipe_send.send(1)
					halted = True
					complete = True

				# regardless of halt file, continue to process responses
				#   sent by the workers (if any are currently available)
				found_something = True
				while found_something:
					if time.time() - last_stats_and_halt > 1.0:
						last_stats_and_halt = time.time()
						# check for halt file
						if not halted:
							if halt_file_path.is_file():
								print(f"\nfound halt file [{halt_file_path}], stopping...")
								# signal to the threads that they should stop
								halt_pipe_send.send(1)
								halted = True
						# print stats
						if compl_worker_jobs > 0:
							time_elapsed = time.time() - start_eta_time
							seconds_per_thread = (time_elapsed + previous_total_elapsed_sec) / float(compl_worker_jobs)
							threads_remaining = float(total_worker_jobs - compl_worker_jobs)
							seconds_remaining = timedelta(seconds=round(seconds_per_thread * threads_remaining))
							pct_complete = (float(compl_worker_jobs) * 100.0) / float(total_worker_jobs)
							total_seconds = round((seconds_per_thread * threads_remaining) + time_elapsed + previous_total_elapsed_sec)
							total_seconds_delta = timedelta(seconds=round(total_seconds))
							print(f'    {round(pct_complete,4)}% complete, ETA:[{seconds_remaining}], total:[{total_seconds_delta}], counting for n={args.n}:[{n_counts[args.n]}], outstanding threads:[{total_worker_jobs}-{compl_worker_jobs}={round(threads_remaining)}]       ', end='\r')
					try:
						data = response_queue.get(block = True, timeout = 1.0)
						# to send minimal data back and forth, for the 0th
						#   item in the tuple we will use:
						# True  - a finished array of counts
						# False - an unfinished Polycube to write to disk
						if data[0]:
							for n,count in enumerate(data[1]):
								n_counts[n] += count
							last_count_increment_time = time.perf_counter()
							compl_worker_jobs += 1
						# these are the abandoned Polycubes, not yet evaluated,
						#   that will be written to disk so they can be read and
						#   resumed later
						else:
							if data[1] is None:
								print("\nthe initial delegator worker was halted.  thus, we cannot resume from this point later and will not write any data to disk")
							else:
								# write the found polycube data (data[1]) to file
								saved_worker_jobs += 1
								polycubes_to_write_to_disk.append(list(data[1].cubes.keys()))
					except QueueEmpty:
						found_something = False
				# might not need this sleep due to the above response_queue.get(timeout = 1.0)
				time.sleep(1)
			# might not need this block, because response_queue.empty() is checked
			#   by the above block
			#while not response_queue.empty():
			#	resp = response_queue.get()
			#	print(f"received response: {resp}")
			# the workers should all be done by this point, but might as well .join() them
			for p in processes:
				p.join()
		print(f"all threads have completed: {compl_worker_jobs=}, {saved_worker_jobs=} (compl+saved @n={args.spawn_n} should be {total_worker_jobs=})")
		if len(polycubes_to_write_to_disk) > 0:
			write_resume_file(
				args.n,
				args.spawn_n,
				polycubes_to_write_to_disk,
				n_counts,
				previous_total_elapsed_sec + (last_count_increment_time - start_time)
			)


	print_results(complete)
	if last_count_increment_time is None:
		last_count_increment_time = time.perf_counter()
	if resume_file_path is None:
		print(f'elapsed seconds: {last_count_increment_time - start_time}')
	else:
		total_seconds = previous_total_elapsed_sec + last_count_increment_time - start_time
		print(f'elapsed seconds: {last_count_increment_time - start_time} + {previous_total_elapsed_sec} from resume file = {total_seconds} total seconds')
