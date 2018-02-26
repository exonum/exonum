module.exports = function(grunt) {
    require('load-grunt-tasks')(grunt);

    grunt.initConfig({
        clean: {
            build: {
                src: ['dist']
            }
        },
        watch: {
            scripts: {
                files: ['./tags/**/*.tag'],
                tasks: ['riot'],
                options: {
                    spawn: false
                }
            }
        },
        riot: {
            options: {
                concat: true
            },
            dist: {
                src: 'tags/*.tag',
                dest: 'dist/app.js'
            }
        }
    });

    grunt.registerTask('default', ['clean', 'riot']);
};
