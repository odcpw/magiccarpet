if(DEFINED MC2_FRAME_TRACE_INJECTED)
    return()
endif()
set(MC2_FRAME_TRACE_INJECTED TRUE)

if(NOT DEFINED MC2_FRAME_TRACE_ROOT)
    message(FATAL_ERROR "MC2_FRAME_TRACE_ROOT must name the private trace tool")
endif()

function(mc2_trace_copy_property source_target destination_target property_name)
    get_target_property(property_value "${source_target}" "${property_name}")
    if(NOT property_value STREQUAL "property_value-NOTFOUND" AND NOT property_value STREQUAL "")
        set_property(TARGET "${destination_target}" PROPERTY "${property_name}" "${property_value}")
    endif()
endfunction()

function(mc2_add_frame_trace)
    if(NOT TARGET remc2)
        message(FATAL_ERROR "The stock remc2 target was not created")
    endif()

    get_target_property(remc2_source_dir remc2 SOURCE_DIR)
    get_target_property(remc2_sources remc2 SOURCES)
    set(trace_sources)
    set(trace_source_list "")
    foreach(source IN LISTS remc2_sources)
        if(IS_ABSOLUTE "${source}")
            set(resolved_source "${source}")
        else()
            cmake_path(
                ABSOLUTE_PATH source
                BASE_DIRECTORY "${remc2_source_dir}"
                OUTPUT_VARIABLE resolved_source
            )
        endif()
        list(APPEND trace_sources "${resolved_source}")
        string(APPEND trace_source_list "${resolved_source}\n")
    endforeach()

    add_executable(remc2-frame-trace ${trace_sources})
    foreach(property_name IN ITEMS
        INCLUDE_DIRECTORIES
        COMPILE_DEFINITIONS
        COMPILE_OPTIONS
        COMPILE_FEATURES
        LINK_DIRECTORIES
        LINK_OPTIONS
        LINK_LIBRARIES
    )
        mc2_trace_copy_property(remc2 remc2-frame-trace "${property_name}")
    endforeach()
    set_target_properties(remc2-frame-trace PROPERTIES
        OUTPUT_NAME "remc2-frame-trace"
    )
    file(
        WRITE "${CMAKE_CURRENT_BINARY_DIR}/frame-trace-source-list.txt"
        "${trace_source_list}"
    )
    add_custom_command(TARGET remc2-frame-trace POST_BUILD
        COMMAND "${CMAKE_COMMAND}" -E copy_directory
            "${CMAKE_SOURCE_DIR}/remc2/font"
            "$<TARGET_FILE_DIR:remc2-frame-trace>/font"
        VERBATIM
    )
endfunction()

cmake_language(DEFER DIRECTORY "${CMAKE_SOURCE_DIR}" CALL mc2_add_frame_trace)
