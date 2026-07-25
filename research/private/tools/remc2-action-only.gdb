set pagination off
set confirm off
set debuginfod enabled off
set env SDL_AUDIODRIVER dummy

# CountFailedRegressionTests calls run_regtest for every level and then for
# both after-load fixtures. Return success immediately for every invocation
# except testType 2, so the recorded 25-frame action fixture runs alone in a
# fresh process without editing the pinned upstream checkout.
break regression-tests.cpp:6
commands
  silent
  if testType != 2
    return 0
  else
    printf "Running isolated action regression: level=%d index=%d steps=%d\n", level, index, maxSteps
  end
  continue
end

run
