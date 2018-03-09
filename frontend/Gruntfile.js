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
                files: ['./src/**/*.*'],
                tasks: ['browserify'],
                options: {
                    spawn: false
                }
            }
        },
        browserify: {
            options: {
                transform: [
                    ['vueify'],
                    ['babelify', {presets: 'es2015'}]
                ]
            },
            dist: {
                src: './src/app.js',
                dest: './dist/build.js'
            }
        }
    });

    grunt.registerTask('default', ['clean', 'browserify']);
};
