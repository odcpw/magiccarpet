set pagination off
set confirm off
set debuginfod enabled off
set env SDL_AUDIODRIVER dummy

# Run only the type-1 after-load idle fixture in a fresh process. This
# distinguishes a fixture/loader mismatch from contamination by earlier
# level regressions in the same remc2-regression-test process.
break regression-tests.cpp:6
commands
  silent
  if testType != 1
    return 0
  else
    printf "Running isolated after-load idle regression: level=%d index=%d steps=%d\n", level, index, maxSteps
  end
  continue
end

run
