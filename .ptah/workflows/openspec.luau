--!strict
-- Shim: openspec lifecycle (groom/implement/verify) over the shared
-- library (see factory-components/README.md for the consumption
-- model). The only workflow code this repo owns.
--
-- ptah scripts take no runtime arguments, so the op and change name
-- are hand-edited per use; one call active at a time.

local piAgent = ptah.agent("pi")

-- COMPONENTS
-- .ptah/libs is mounted by the devshell from patextreme/ptah's
-- factory-components source tree.
local openspec = require("../libs/components/openspec/component")
local prReview = require("../libs/components/pr-review-loop/component")
local opsx = openspec.new({
	agent = piAgent,
	judgeAgent = piAgent,
	model = "zai/glm-5.3",
	judgeModel = "zai/glm-5.3-flash",
})
local prr = prReview.new({
	agent = piAgent,
	judgeAgent = piAgent,
	model = "zai/glm-5.3",
	judgeModel = "zai/glm-5.3-flash",
	dryRun = false,
	maxIterations = 15,
})

-- WORKFLOW
local changeName = "pr-review-instruction-contract"

opsx:groom(changeName)
opsx:implement(changeName)
opsx:verify(changeName)

local session = piAgent:session({
	id = "committer",
	resultSchema = { prUrl = { type = "string" } }
})
session:prompt("Commit the changes and push to remote. Exclude changes in .ptah directory.")
local result = session:prompt("Create a PR in draft state. Then pass the url back to the ptah via result submit tool")
prr:review(result.result.prUrl :: string )
