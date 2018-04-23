module.exports = function(grunt) {
  require('load-grunt-tasks')(grunt);

  grunt.initConfig({
    clean: {
      build: {
        src: ['dist']
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
    },
    watch: {
      scripts: {
        files: ['./src/**/*'],
        tasks: ['browserify'],
        options: {
          spawn: false
        }
      }
    }
  });

  grunt.registerTask('default', ['clean', 'browserify']);
};
