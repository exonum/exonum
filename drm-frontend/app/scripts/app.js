var DRMRouter = Backbone.Router.extend({
    routes: {
      // Main pages
      ""                      : "welcome",
      "login"                 : "login",
      "registration"          : "registration",

      // Blockchain Explorer
      "blockchain"            : "blockchain",
      "blockchain/:page"      : "blockchain",
      "block/:height"         : "block",

      // DRM
      "dashboard"             : "dashboard",
      "content/:fingerprint"  : "content",
      "add-report/:fingerprint" : "addReport",
      "add-content"           : "addContent"
    },

    // Main pages

    welcome: function() {
      app.views.container.changePage('welcome');
    },

    login: function() {
      app.views.login.render();
      app.views.container.changePage('login');
    },

    registration: function() {
      app.views.registration.render();
      app.views.container.changePage('registration');
    },

    // Blockchain Explorer

    blockchain: function(height) {
      app.views.container.loadingStart();

      var requestData = {count: 15};

      if (height) {requestData.from = height;}

      app.blocks.fetch({
        data: requestData,
        success: function() {
          app.last_height = app.blocks.isEmpty() ? 0 : app.blocks.at(0).get('height');
          app.views.blockchain.render();
          app.views.container.changePage('blockchain');
        },
        error: function() {
          app.views.container.changePage('error');
        }
      });
    },

    block: function(height) {
      app.views.container.loadingStart();

      var d1 = new Block({height: height}).fetch({
        success: function(model) {
          app.views.block.model = model;
        }
      });

      var d2 = new Blocks().fetch({
        data: {count: 1},
        success: function(blocks) {
          app.last_height = blocks.isEmpty() ? 0 : blocks.at(0).get('height');
        }
      });

      var def = $.when(d1, d2);

      def.done(function() {
        app.views.block.render();
        app.views.container.changePage('block');
      });
      def.fail(function() {
        app.views.container.changePage('error');
     });
    },

    // DRM

    dashboard: function() {
      app.views.container.loadingStart();
      if (!app.user) {
        this.navigate('login', {trigger: true});
      } else {
        if (app.user.get('role') == "owner") {
          app.views.ownerDashboard.render();
          app.views.container.changePage("ownerDashboard");
        } else {
          app.views.distributorDashboard.render();
          app.views.container.changePage("distributorDashboard");
        }
      }
    },

    content: function(fingerprint) {
      app.views.container.loadingStart();
      var content = new Content({
        fingerprint: fingerprint
      });

      content.fetch({
        success: function(model) {
          app.views.content.model = model;
          app.views.content.render();
          app.views.container.changePage("content");
        },
        error: function() {
          app.onError("Content with given fingerprint not found");
        }
      });
    },

    addContent: function() {
      app.views.container.loadingStart();
      if (!app.user || app.user.get("role") != "owner") {
        return this.navigate('login', {trigger: true});
      }
      app.views.addContent.render();
      app.views.container.changePage('addContent');
    },

    addReport: function(fingerprint) {
      app.views.container.loadingStart();
      if (!app.user || app.user.get("role") != "distributor") {
        return this.navigate('login', {trigger: true});
      }

      var content = new Content({
        fingerprint: fingerprint
      });

      content.fetch({
        success: function(model) {
          app.views.addReport.model = model;
          app.views.addReport.render();
          app.views.container.changePage("addReport");
        },
        error: function() {
          app.onError("Content with given fingerprint not found");
        }
      });
    }
});

var app = {

  router: new DRMRouter(),

  initialize: function() {
    this.last_height = 0;
    this.blocks = new Blocks();
    this.users = [];
    this.views = {
      container: new ContainerView(),
      // navbar: new NavbarView(),
      // alert: new AlertView(),
      welcome: new WelcomePage(),
      login: new LoginPage(),
      registration: new RegistrationPage(),
      blockchain: new BlockchainPage(),
      block: new BlockPage(),
      ownerDashboard: new OwnerDashboardPage(),
      distributorDashboard: new DistributorDashboardPage(),
      content: new ContentPage(),
      addContent: new AddContentPage(),
      addReport: new AddReportPage()
    };
    Backbone.history.start();
    alertify.maxLogItems(10);
  },

  login: function(user) {
    app.views.container.loadingStart();
    new Auth().failoverSave({
      pub_key: user.pub_key,
      sec_key: user.sec_key
    }, {
      retries: 20,
      timeout: 500,
      success: function(model) {
        app.user = model;
        app.views.container.updateUser();
        app.router.navigate("/dashboard", {trigger: true});
      },
      error: app.onError("Authentification failed")
    });
  },

  registration: function(role, name, callback) {
    app.views.container.loadingStart();
    var Model = role == 'owner' ? Owner : Distributor;
    new Model().save({name: name}, {
      success: function(model, response) {
        new Auth().failoverSave({
          pub_key: response.pub_key,
          sec_key: response.sec_key
        }, {
          retries: 20,
          timeout: 500,
          success: function(model) {
            // add new user to localStorage
            var users = JSON.parse(localStorage.getItem('users')) || [];
            users.push(model.attributes);
            localStorage.setItem('users', JSON.stringify(users));

            app.users.push(model.attributes);
            app.user = model;
            app.views.container.updateUser();
            app.router.navigate("/dashboard", {trigger: true});

            if (model.get('role') == 'owner') {
              app.owners.push({
                id: model.get('id'),
                name: model.get('name')
              });
            }

            callback();

            alertify.success('You have created ' + role + ' account');
          },
          error: app.onError("Authentification failed")
        });
      },
      error: function() {
        app.views.container.changePage('error');
      }
    });
  },

  addContent: function(content) {
    app.views.container.loadingStart();
    new Content().save(content, {
      url: Content.prototype.urlRoot,
      success: function(model) {
        model.failoverFetch({
          retries: 20,
          timeout: 500,
          success: function() {
            new Auth().save({
              pub_key: app.user.get("pub_key"),
              sec_key: app.user.get("sec_key")
            }, {
              success: function(model) {
                app.user = model;
                app.router.navigate('dashboard', {trigger: true});

                alertify.success('You have added new content');
              },
              error: app.onError("Unable to create new content")
            });
          },
          error: app.onError("Unable to create new content")
        });
      },
      error: app.onError("Unable to create new content")
    });
  },

  addContract: function(fingerprint) {
    app.views.container.loadingStart();
    new Contract().save({
        fingerprint: fingerprint
      }, {
      success: function() {
        function waitForContract() {
          var content = new Content({
            fingerprint: fingerprint
          });
          content.fetch({
            success: function(model) {
              var ready = model.get("contract");
              console.log("ready", ready);
              if (ready) {
                new Auth().save({
                  pub_key: app.user.get("pub_key"),
                  sec_key: app.user.get("sec_key")
                }, {
                  success: function(model) {
                    app.user = model;
                    app.router.navigate('dashboard', {trigger: true});

                    alertify.success('You have purchased content');
                  },
                  error: app.onError("Unable to create new content")
                });

              } else {
                setTimeout(waitForContract, 500);
              }
            },
            error: function() {
              app.onError("Content with given fingerprint not found");
            }
          });
        }
        waitForContract();
      },
      error: app.onError("Unable to create new content")
    });
  },

  addReport: function(report) {
    console.log("report", report);
    app.views.container.loadingStart();
    new Report().save(report, {
      type: 'PUT',
      url: Report.prototype.urlRoot,
      success: function(model) {
        model.failoverFetch({
          retries: 20,
          timeout: 500,
          success: function() {
            new Auth().save({
              pub_key: app.user.get("pub_key"),
              sec_key: app.user.get("sec_key")
            }, {
              success: function(model) {
                app.user = model;
                app.router.navigate('dashboard', {trigger: true});

                alertify.success('You have updated distribution status');
              },
              error: app.onError("Unable to create new content")
            });
          },
          error: app.onError("Unable to add new report")
        });
      },
      error: app.onError("Unable to add new report")
    });
  },

  onError: function(messageTitle, callback) {
    return function(model, response) {
      if (callback !== undefined) {
        callback();
      }
      if (response.responseJSON !== undefined && response.responseJSON.message !== undefined) {
        app.views.alert.error(messageTitle, response.responseJSON.message);
        app.views.container.loadingFinish();
      } else {
        app.onFatalError();
      }
    };
  },

  onFatalError: function() {
    app.views.container.changePage('error');
  }

};

$(function() {
  app.initialize();
});
