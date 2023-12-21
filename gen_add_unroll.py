# generates rust code for use in Polycube.add()

# minus x, plus x, minus y, plus y, minus z, plus z
directions = [0, 1, 2, 3, 4, 5]
# used to create a unique integer position for each cube
#   in a polycube
direction_costs = [-1, 1, -100, 100, -10_000, 10_000]

# direction = 0 -> direction cost = -1
# direction = 1 -> direction cost = 1
# direction = 2 -> direction cost = -100
# direction = 3 -> direction cost = 100
# direction = 4 -> direction cost = -10_000
# direction = 5 -> direction cost = 10_000
for direction in directions:
	direction_cost = direction_costs[direction]
	print(f"""
		// direction = {direction} -> direction cost = {direction_cost}
		{'let ' if direction == 0 else ''}neighbor_pos = pos {'+' if direction_cost > 0 else '-'} {abs(direction_cost)};
		match self.cube_info_by_pos.get_mut(&neighbor_pos) {{
			Some(neighbor_info) => {{
				new_info[{direction}] = Some(neighbor_pos);
				new_enc |= {1 << (5-direction)};
				neighbor_info[{direction ^ 1}] = Some(pos);
				neighbor_info[6] = Some(neighbor_info[6].unwrap() | {1 << ((5-direction) ^ 1)});
			}}
			None => {{}}
		}}""", end='')
