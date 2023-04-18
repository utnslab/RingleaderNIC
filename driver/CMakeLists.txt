cmake_minimum_required(VERSION 2.8)
project(ixy C)

set(CMAKE_C_STANDARD 11)
SET(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -flto")
SET(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -flto")

add_compile_options(-g -O3 -march=native -fomit-frame-pointer -std=c11
	-D_XOPEN_SOURCE=700
	-D_DEFAULT_SOURCE
	-Wall
	-Wextra
	-Wno-unused-parameter
	-Wno-unused-function
	-Wformat=2
	-flto
)

include_directories(
	${CMAKE_CURRENT_SOURCE_DIR}/src
)

set(CMAKE_THREAD_PREFER_PTHREAD TRUE)
set(THREADS_PREFER_PTHREAD_FLAG TRUE)
find_package(Threads REQUIRED)

set(SOURCE_COMMON src/pci.c src/memory.c src/msg.c  src/stats.c src/interrupts.c src/driver/device.c src/driver/mqnic.c src/libixy-vfio.c)

ADD_LIBRARY(dynamicixy SHARED 
             ${SOURCE_COMMON} )

add_executable(ixy_testmmio src/app/ixy_testmmio.c ${SOURCE_COMMON})
add_executable(ixy-throughput src/app/ixy-throughput.c ${SOURCE_COMMON})
target_link_libraries(ixy_testmmio Threads::Threads)
target_link_libraries(ixy-throughput Threads::Threads)
# target_link_libraries(ixy-pcap Threads::Threads)
# add_library(ixy-vfio SHARED src/libixy-vfio.c)

