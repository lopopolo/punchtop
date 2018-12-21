const HtmlWebPackPlugin = require("html-webpack-plugin");
const HtmlWebpackInlineSourcePlugin = require("html-webpack-inline-source-plugin");
const MiniCssExtractPlugin = require("mini-css-extract-plugin");

const devMode = process.env.NODE_ENV !== 'production'

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
      useShortDoctype: true,
    }
  }),
  new HtmlWebpackInlineSourcePlugin()
];

module.exports = {
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
            loader: devMode ? "style-loader" : MiniCssExtractPlugin.loader,
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
        use: ["style-loader", "css-loader"]
      },
      {
        test: /\.(png|jp(e?)g|svg)$/,
        use: "url-loader",
      }
    ]
  },
  plugins
};
