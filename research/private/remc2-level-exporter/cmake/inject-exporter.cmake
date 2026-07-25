if(DEFINED MC2_LEVEL_EXPORTER_INJECTED)
    return()
endif()
set(MC2_LEVEL_EXPORTER_INJECTED TRUE)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_LIST_FILE}"
)

if(NOT DEFINED MC2_LEVEL_EXPORTER_ROOT)
    message(FATAL_ERROR "MC2_LEVEL_EXPORTER_ROOT must name the private exporter source directory")
endif()

function(mc2_copy_target_property source_target destination_target property_name)
    get_target_property(property_value "${source_target}" "${property_name}")
    if(NOT property_value STREQUAL "property_value-NOTFOUND" AND NOT property_value STREQUAL "")
        set_property(TARGET "${destination_target}" PROPERTY "${property_name}" "${property_value}")
    endif()
endfunction()

function(mc2_add_level_exporter)
    if(NOT TARGET remc2)
        message(FATAL_ERROR "The stock remc2 target was not created")
    endif()

    get_target_property(remc2_source_dir remc2 SOURCE_DIR)
    get_target_property(remc2_sources remc2 SOURCES)
    set(exporter_sources)
    foreach(source IN LISTS remc2_sources)
        if(IS_ABSOLUTE "${source}")
            set(resolved_source "${source}")
        else()
            cmake_path(ABSOLUTE_PATH source BASE_DIRECTORY "${remc2_source_dir}" OUTPUT_VARIABLE resolved_source)
        endif()
        get_filename_component(source_name "${resolved_source}" NAME)
        if(NOT source_name STREQUAL "remc2.cpp")
            list(APPEND exporter_sources "${resolved_source}")
        endif()
    endforeach()
    list(APPEND exporter_sources "${MC2_LEVEL_EXPORTER_ROOT}/src/exporter_main.cpp")

    execute_process(
        COMMAND git -C "${CMAKE_SOURCE_DIR}" status --porcelain=v1 --untracked-files=all
        OUTPUT_VARIABLE upstream_status
        OUTPUT_STRIP_TRAILING_WHITESPACE
        COMMAND_ERROR_IS_FATAL ANY
    )
    execute_process(
        COMMAND git -C "${CMAKE_SOURCE_DIR}" diff --binary --no-ext-diff HEAD
        OUTPUT_VARIABLE upstream_binary_diff
        COMMAND_ERROR_IS_FATAL ANY
    )
    string(SHA256 upstream_diff_sha256 "${upstream_binary_diff}")
    set(modified_json)
    set(modified_files_json)
    string(REPLACE "\n" ";" status_lines "${upstream_status}")
    foreach(status_line IN LISTS status_lines)
        if(status_line STREQUAL "")
            continue()
        endif()
        string(SUBSTRING "${status_line}" 3 -1 modified_path)
        string(REPLACE "\\" "\\\\" modified_path "${modified_path}")
        string(REPLACE "\"" "\\\"" modified_path "${modified_path}")
        if(NOT "${modified_json}" STREQUAL "")
            string(APPEND modified_json ",")
        endif()
        string(APPEND modified_json "\"${modified_path}\"")
        if(EXISTS "${CMAKE_SOURCE_DIR}/${modified_path}" AND
           NOT IS_DIRECTORY "${CMAKE_SOURCE_DIR}/${modified_path}")
            file(SHA256 "${CMAKE_SOURCE_DIR}/${modified_path}" modified_sha)
            if(NOT "${modified_files_json}" STREQUAL "")
                string(APPEND modified_files_json ",")
            endif()
            string(APPEND modified_files_json
                "{\"path\":\"${modified_path}\",\"sha256\":\"${modified_sha}\"}")
        endif()
    endforeach()

    set(source_rows)
    foreach(source IN LISTS exporter_sources)
        if(EXISTS "${source}" AND NOT IS_DIRECTORY "${source}")
            set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${source}")
            file(SHA256 "${source}" source_sha)
            file(RELATIVE_PATH logical_path "${CMAKE_SOURCE_DIR}" "${source}")
            if(logical_path MATCHES "^\\.\\.")
                file(RELATIVE_PATH logical_path "${MC2_LEVEL_EXPORTER_ROOT}" "${source}")
                set(logical_path "private-exporter/${logical_path}")
            endif()
            string(REPLACE "\\" "/" logical_path "${logical_path}")
            list(APPEND source_rows "${logical_path}|${source_sha}")
        endif()
    endforeach()
    list(SORT source_rows)

    set(source_json)
    set(source_identity_input)
    foreach(source_row IN LISTS source_rows)
        string(REPLACE "|" ";" source_parts "${source_row}")
        list(GET source_parts 0 logical_path)
        list(GET source_parts 1 source_sha)
        if(NOT "${source_json}" STREQUAL "")
            string(APPEND source_json ",")
        endif()
        string(APPEND source_json "{\"path\":\"${logical_path}\",\"sha256\":\"${source_sha}\"}")
        string(APPEND source_identity_input "${logical_path}\\0${source_sha}\\n")
    endforeach()
    string(SHA256 source_identity "${source_identity_input}")

    set(provenance_header "${CMAKE_CURRENT_BINARY_DIR}/mc2_exporter_source_provenance.h")
    file(WRITE "${provenance_header}"
        "#pragma once\n"
        "#define MC2_COMPILED_SOURCE_SHA256 \"${source_identity}\"\n"
        "#define MC2_UPSTREAM_DIFF_SHA256 \"${upstream_diff_sha256}\"\n"
        "#define MC2_UPSTREAM_STATUS_JSON R\"MC2JSON([${modified_json}])MC2JSON\"\n"
        "#define MC2_MODIFIED_FILE_HASHES_JSON R\"MC2JSON([${modified_files_json}])MC2JSON\"\n"
        "#define MC2_SOURCE_INVENTORY_JSON R\"MC2JSON([${source_json}])MC2JSON\"\n"
    )

    add_executable(remc2-level-exporter ${exporter_sources})
    foreach(property_name IN ITEMS
        INCLUDE_DIRECTORIES
        COMPILE_DEFINITIONS
        COMPILE_OPTIONS
        COMPILE_FEATURES
        LINK_DIRECTORIES
        LINK_OPTIONS
        LINK_LIBRARIES
    )
        mc2_copy_target_property(remc2 remc2-level-exporter "${property_name}")
    endforeach()

    target_include_directories(remc2-level-exporter PRIVATE
        "${CMAKE_CURRENT_BINARY_DIR}"
        "${remc2_source_dir}"
        "${remc2_source_dir}/engine"
        "${remc2_source_dir}/portability"
    )
    target_compile_definitions(remc2-level-exporter PRIVATE
        MC2_PRIVATE_ROOT="${MC2_PRIVATE_ROOT}"
        MC2_UPSTREAM_COMMIT="${MC2_UPSTREAM_COMMIT}"
    )
    add_custom_command(TARGET remc2-level-exporter POST_BUILD
        COMMAND "${CMAKE_COMMAND}" -E copy_directory
            "${CMAKE_SOURCE_DIR}/remc2/font"
            "$<TARGET_FILE_DIR:remc2-level-exporter>/font"
        VERBATIM
    )
endfunction()

cmake_language(DEFER DIRECTORY "${CMAKE_SOURCE_DIR}" CALL mc2_add_level_exporter)
