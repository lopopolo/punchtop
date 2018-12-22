const path = require("path");
const HtmlWebPackPlugin = require("html-webpack-plugin");
const HtmlWebpackInlineSourcePlugin = require("html-webpack-inline-source-plugin");
const UglifyJsPlugin = require("uglifyjs-webpack-plugin");
const MiniCssExtractPlugin = require("mini-css-extract-plugin");
const OptimizeCSSAssetsPlugin = require("optimize-css-assets-webpack-plugin");

const plugins = [
  new MiniCssExtractPlugin({
    filename: "[name].css",
    chunkFilename: "[id].css"
  }),
  new HtmlWebPackPlugin({
    template: "./src/index.html",
    filename: "./index.html",
    inlineSource: /\.(js|css)$/,
    minify: {
      collapseWhitespace: true,
      minifyCSS: true,
      minifyJS: true,
      removeComments: true,
      useShortDoctype: true
    }
  }),
  new HtmlWebpackInlineSourcePlugin()
];

module.exports = (env, argv) => {
  let target = "debug";
  let cssLoader = "style-loader";
  if (argv.mode === "production") {
    target = "release";
    cssLoader = MiniCssExtractPlugin.loader;
  }
  return {
    context: path.resolve(__dirname),
    output: {
      path: path.resolve(__dirname, `target/${target}`)
    },
    module: {
      rules: [
        {
          test: /\.jsx?$/,
          exclude: /node_modules/,
          use: {
            loader: "babel-loader"
          }
        },
        {
          test: /\.css$/,
          exclude: /index\.css/,
          use: [
            {
              loader: cssLoader
            },
            {
              loader: "css-loader",
              options: {
                modules: true,
                importLoaders: 1,
                localIdentName: "[name]_[local]_[hash:base64]"
              }
            }
          ]
        },
        {
          test: /\.css$/,
          include: /index\.css/,
          use: [cssLoader, "css-loader"]
        },
        {
          test: /\.(png|jp(e?)g|svg)$/,
          use: "url-loader"
        }
      ]
    },
    plugins,
    optimization: {
      minimizer: [
        new UglifyJsPlugin({
          cache: true,
          parallel: true
        }),
        new OptimizeCSSAssetsPlugin()
      ]
    }
  };
};
