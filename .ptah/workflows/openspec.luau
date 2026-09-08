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
local changeName = "add-midnight-vc-families"

opsx:groom(changeName)
opsx:implement(changeName)
opsx:verify(changeName)

local session = piAgent:session({
	id = "committer",
	resultSchema = {
		type = "object",
		properties = { prUrl = { type = "string" } },
		required = { "prUrl" },
	},
})
session:prompt("Commit the changes and push to remote. Exclude changes in .ptah directory.")

-- Typed results degrade to nil by design; retry the PR-creation turn
-- instead of passing a nil URL into the review loop.
local prUrl: string? = nil
for attempt = 1, 3 do
	local result = session:prompt(
		("Create a PR in draft state. Then pass the url back to ptah via the result submit tool as the `prUrl` field (attempt %d of 3)."):format(attempt)
	)
	if result.result ~= nil and result.result.prUrl ~= nil then
		prUrl = result.result.prUrl
		break
	end
	ptah.log("committer did not submit prUrl; retrying")
end
if prUrl == nil then
	error("committer never submitted prUrl; aborting before pr-review", 0)
end
prr:review(prUrl)
