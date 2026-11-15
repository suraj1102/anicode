-- A derive can read other shapes. The follower stays below the leader.
local leader = Circle { r = 0.5, pos = vec(-5, 1.5), fill = BLUE }
local follower = Rect { w = 0.6, h = 0.6, fill = ORANGE }
follower.x = derive(function() return leader.x:get() end)
follower.y = derive(function() return leader.y - 3 end)

at "right" { move_to(leader, vec(5, 2)) }
at "left" { move_to(leader, vec(0, 0.5)) }
