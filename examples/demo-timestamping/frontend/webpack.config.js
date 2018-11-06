const path = require('path')
const { VueLoaderPlugin } = require('vue-loader')
require('babel-polyfill')

module.exports = {
  mode: 'development',
  entry: [
    'babel-polyfill',
    './src/app'
  ],
  output: {
    path: path.resolve(__dirname, 'dist'),
    publicPath: '/dist/',
    filename: 'build.js'
  },
  module: {
    rules: [
      {
        test: /\.js/,
        use: 'babel-loader'
      },
      {
        test: /\.vue$/,
        use: 'vue-loader'
      },
      {
        test: /\.css$/,
        use: [
          'vue-style-loader',
          'css-loader'
        ]
      }
    ]
  },
  plugins: [
    new VueLoaderPlugin()
  ]
}
