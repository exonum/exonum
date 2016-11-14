var WelcomePage = Backbone.View.extend({
  title: 'DRM Demo',
  showToolbar: false,
  backPage: undefined,

  el: '.page[data-page="welcome"]',

  events: {
    'click #proceed-demo': 'proceedDemo'
  },

  proceedDemo: function() {
    app.router.navigate('login', {trigger: true});
  }
});

var LoginPage = Backbone.View.extend({
  title: "Login",
  showToolbar: true,
  backPage: undefined,

  el: '.page[data-page="login"]',

  template: templates.login,

  events: {
    'click .login': 'login',
    'click #login-registration': 'registration',
    'click #login-blockchain': 'blockchain',
    'click #login-flow': 'flow'
  },

  login: function(e) {
    var index = $(e.currentTarget).data('index');
    app.login(app.users[index]);
  },

  registration: function() {
    app.router.navigate('registration', {trigger: true});
  },

  blockchain: function() {
    app.router.navigate('blockchain', {trigger: true});
  },

  flow: function() {
    app.router.navigate("flow", {trigger: true});
  },

  render: function() {
    app.users = JSON.parse(localStorage.getItem('users')) || [];
    this.$el.html(this.template({users: app.users}));
    return this;
  }
});

var RegistrationPage = Backbone.View.extend({
  title: 'Registration',
  showToolbar: true,
  backPage: 'login',

  el: '.page[data-page="registration"]',

  events: {
    'click #registration-submit': 'registrationSubmit',
    'focus #registration-name': 'focusName'
  },

  focusName: function() {
    this.$el.find('#registration-name-form-group').removeClass('has-error');
  },

  registrationSubmit: function() {
    var nameField = this.$el.find('#registration-name');
    var name = $.trim(nameField.val());
    var role = this.$el.find('#registration-form input[type=radio]:checked').val();

    // reset name input
    function callback() {
      nameField.val('');
    }

    if (!name) {
      this.$el.find('#registration-name-form-group').addClass('has-error');
      alertify.error('Name is not defined');
      return;
    }

    app.registration(role, name, callback);
  },

  render: function() {
  }
});

var BlockchainPage = Backbone.View.extend({
  title: 'Blockchain Explorer',
  showToolbar: true,
  backPage: 'login',

  el: '.page[data-page="blockchain"]',

  template: templates.blockchain,

  events: {
    'click .blockchain tbody tr': 'showBlock',
    'click #blockchain-page-prev': 'prevPage',
    'click #blockchain-page-next': 'nextPage',
    'click #blockchain-refresh': 'refresh'
  },

  showBlock: function(e) {
    var height = $(e.target).parents('tr').data('block');
    app.router.navigate('block/' + height, {trigger: true});
  },

  prevPage: function() {
    var currentHeight = app.blocks.at(0).get('height');
    var newHeight = currentHeight - 15;

    if (currentHeight == 14) {
      return false;
    } else if (newHeight <= 14) {
      newHeight = 14;
    }

    app.router.navigate('blockchain/' + newHeight, {trigger: true});
  },

  nextPage: function() {
    var currentHeight = app.blocks.at(0).get('height');
    var newHeight = currentHeight + 15;

    if (newHeight < app.newestHeight) {
      app.router.navigate('blockchain/' + newHeight, {trigger: true});
    } else {
      app.router.navigate('blockchain', {trigger: true});
    }
  },

  refresh: function() {
    app.router.blockchain();
  },

  render: function() {
    this.$el.html(this.template({
      blocks: app.blocks,
      lastHeight: app.lastHeight,
      newestHeight: app.newestHeight
    }));
    return this;
  }
});

var BlockPage = Backbone.View.extend({
  title: function() {
    return 'Block #' + this.model.get('height')
  },
  showToolbar: true,
  backPage: 'blockchain',

  el: '.page[data-page="block"]',

  template: templates.block,

  events: {
    'click #block-prev': 'prevBlock',
    'click #block-next': 'nextBlock'
  },

  prevBlock: function() {
    var height = this.model.get('height') + 1;
    app.router.navigate('block/' + height, {trigger: true});
  },

  nextBlock: function() {
    var height = this.model.get('height') - 1;
    app.router.navigate('block/' + height, {trigger: true});
  },

  render: function() {
    this.$el.html(this.template({
      block: this.model,
      lastHeight: app.lastHeight
    }));
    return this;
  }
});

var OwnerDashboardPage = Backbone.View.extend({
  title: 'Owner Dashboard',
  showToolbar: true,
  backPage: 'login',

  el: '.page[data-page="ownerDashboard"]',

  template: templates.ownerDashboard,

  events: {
    'click #owner-dashboard-add-content': 'addContent',
    'click .owned tbody tr': 'showContent'
  },

  addContent: function() {
    app.router.navigate('add-content', {trigger: true});
  },

  showContent: function(e) {
    var fingerprint = $(e.target).parents('tr').data('fingerprint');
    app.router.navigate('content/' + fingerprint, {trigger: true});
  },

  render: function() {
    this.$el.html(this.template({user: app.user}));
    return this;
  }
});

var DistributorDashboardPage = Backbone.View.extend({
  title: 'Distributor Dashboard',
  showToolbar: true,
  backPage: 'login',

  el: '.page[data-page="distributorDashboard"]',

  template: templates.distributorDashboard,

  events: {
    'click .distributed tbody tr': 'showContent',
    'click .available tbody tr': 'showContent'
  },

  showContent: function(e) {
    var fingerprint = $(e.currentTarget).data('fingerprint');
    app.router.navigate('content/' + fingerprint, {trigger: true});
  },

  render: function() {
    this.$el.html(this.template({user: app.user}));
    return this;
  }
});

var ContentPage = Backbone.View.extend({
  title: function() {
    return this.model.get('title')
  },
  showToolbar: true,
  backPage: 'dashboard',

  el: '.page[data-page="content"]',

  template: templates.content,

  events: {
    'click #content-buy-inside': 'buyContract',
    'click #content-update-status': 'addReport'
  },

  buyContract: function() {
    app.addContract(this.model.get('fingerprint'));
  },

  addReport: function() {
    app.router.navigate('add-report/' + this.model.get('fingerprint'), {trigger: true});
  },

  render: function() {
    this.$el.html(this.template({content: this.model, user: app.user}));
    return this;
  }
});

var AddContentPage = Backbone.View.extend({
  title: 'Add Content',
  showToolbar: true,
  backPage: 'dashboard',

  el: '.page[data-page="addContent"]',

  template: templates.addContent,

  events: {
    'click #add-content-select-file': 'generateFingerprint',
    'click #add-content-publish': 'addContent',
    'focus #add-content-title': 'onFocus',
    'focus #add-content-min-plays': 'onFocus',
    'focus #add-content-price-per-listen': 'onFocus',
    'focus .add-content-owner-id': 'onFocus',
    'focus .add-content-owner-share': 'onFocus',
    'click #add-content-add-coowner': 'addCoowner',
    'click .add-content-remove-coowner': 'removeCoowner'
  },

  onFocus: function(e) {
    $(e.target).parents('.form-group').removeClass('has-error');
  },

  generateFingerprint: function() {
    function generateFingerprint() {
      var result, i, j;
      result = '';
      for(j=0; j<64; j++) {
        i = Math.floor(Math.random()*16).toString(16).toUpperCase();
        result = result + i;
      }
      return result;
    }
    this.$el.find('#add-content-fingerprint').val(generateFingerprint());
    this.$el.find('#add-content-fingerprint-group').removeClass('has-error');
  },

  getOwners: function() {
    var owners = [];
    this.$el.find('.add-content-owner').each(function(i, el) {
      owners.push({
        owner_id: parseInt($(el).find('.add-content-owner-id').val()),
        share: Math.round($(el).find('.add-content-owner-share').val().replace(/[^0-9]/g, ''))
      });
    });
    return owners;
  },

  addCoowner: function() {
    var to = $('#co-owners');
    var tpl = $('#co-owner').html();

    app.views.container.loadingStart();
    to.append(tpl);
    app.views.container.loadingFinish();
  },

  removeCoowner: function(e) {
    $(e.target).parents('.add-content-owner').remove();
  },

  addContent: function() {
    console.log('!! Add content');
    var hasError = false;

    var content = {
      fingerprint: this.$el.find('#add-content-fingerprint').val(),
      title: this.$el.find('#add-content-title').val(),
      price_per_listen: this.$el.find('#add-content-price-per-listen').val(),
      min_plays: this.$el.find('#add-content-min-plays').val(),
      additional_conditions: this.$el.find('#add-content-additional-conditions').val(),
      owners: this.getOwners()
    };

    console.log(content);

    if (!content.fingerprint) {
      hasError = true;
      this.$el.find('#add-content-fingerprint-group').addClass('has-error');
      alertify.error('Fingerprint is not defined');
    }
    if (!content.title) {
      hasError = true;
      this.$el.find('#add-content-title-group').addClass('has-error');
      alertify.error('Title is not defined');
    }
    if (!content.price_per_listen) {
      hasError = true;
      this.$el.find('#add-content-price-per-listen-group').addClass('has-error');
      alertify.error('Price per each play is not defined');
    }
    if (!content.min_plays) {
      hasError = true;
      this.$el.find('#add-content-min-plays-group').addClass('has-error');
      alertify.error('The minimum number of plays is not defined');
    }

    // validate owners
    var shareSum = 0;
    var owners = this.$el.find('.add-content-owner');
    $.each(content.owners, function(i, owner) {
      shareSum += owner.share;

      // check if share is greater than 0
      if (owner.share === 0) {
        hasError = true;
        owners.eq(i).addClass('has-error');
        alertify.error('Share should be greater than zero');
      }

      // check if defined
      if (isNaN(owner.owner_id)) {
        hasError = true;
        owners.eq(i).addClass('has-error');
        alertify.error('Owner is not defined');
      }

      // check if unique
      $.each(content.owners, function(j, o) {
        if (i !== j && owner.owner_id === o.owner_id) {
          hasError = true;
          owners.eq(i).addClass('has-error');
          owners.eq(j).addClass('has-error');
          alertify.error('Owners should be unique');
        }
      });
    });

    // check if total share is 100%
    if (shareSum !== 100) {
      hasError = true;
      owners.addClass('has-error');
      alertify.error('Total share should be 100%');
    }

    if (!hasError) {
      content.price_per_listen = Math.round(content.price_per_listen * 100);
      content.min_plays = Math.round(content.min_plays);
      app.addContent(content);
    }

  },

  initialize: function() {
    var owner = new Owner();
    owner.fetch({
      success: function(model) {
        app.owners = model.values();
      },
      error: function() {
        app.onError('No owners were found');
      }
    });
  },

  render: function() {
    this.$el.html(this.template({
      content: this.model,
      user: app.user,
      owners: app.owners
    }));
    return this;
  }
});

var AddReportPage = Backbone.View.extend({
  title: 'Add Report',
  showToolbar: true,
  backPage: 'dashboard',

  el: '.page[data-page="addReport"]',

  template: templates.addReport,

  events: {
    'click #add-report': 'addReport',
    'focus #add-report-time': 'onFocus',
    'focus #add-report-plays': 'onFocus',
    'focus #add-report-comment': 'onFocus'
  },

  onFocus: function(e) {
    $(e.target).parents('.form-group').removeClass('has-error');
  },

  addReport: function() {
    var hasError = false;

    function generateUUID() {
      var result, i, j;
      result = '';
      for(j=0; j<64; j++) {
        i = Math.floor(Math.random()*16).toString(16).toUpperCase();
        result = result + i;
      }
      return result;
    }

    var report = {
      uuid: generateUUID(),
      fingerprint: this.model.get('fingerprint'),
      time: this.$el.find('#add-report-time').val(),
      plays: this.$el.find('#add-report-plays').val(),
      comment: this.$el.find('#add-report-comment').val()
    };

    console.log(report);

    if (!report.time) {
      hasError = true;
      this.$el.find('#add-report-time-group').addClass('has-error');
      alertify.error('Date is not defined');
    }
    if (!report.plays) {
      hasError = true;
      this.$el.find('#add-report-plays-group').addClass('has-error');
      alertify.error('Number of plays is not defined');
    }
    if (!report.comment) {
      hasError = true;
      this.$el.find('#add-report-comment-group').addClass('has-error');
      alertify.error('Comment is not defined');
    }

    if (!hasError) {
      report.time = Math.round(moment(report.time).format("x"));
      app.addReport(report);
    }

  },

  initialize: function() {
    // hack to provide correct timezone support
    var local = new Date();
    local.setMinutes(local.getMinutes() - local.getTimezoneOffset());
    this.today = local.toJSON().slice(0, 10);
  },

  render: function() {
    this.$el.html(this.template({
        content: this.model,
        user: app.user,
        today: this.today
      })
    );
    return this;
  }
});

var ContainerView = Backbone.View.extend({

  el: 'body',

  events: {
    'click .toolbar-return-button': 'back',
    'click #user': 'showDashboard',
    'click #blockchain-explorer': 'showBlockchain',
    'click #flow': 'showFlow',
    'click': 'collapseMenu',
    'touchstart': 'collapseMenu'
  },

  loadingStart: function() {
    this.loading = true;
    this.render();
  },

  loadingFinish: function() {
    this.loading = false;
    this.render();
  },

  updateUser: function() {
    if (app.user) {
      this.$el.find('#menu').removeClass('hidden');
      this.$el.find('#user').removeClass('hidden').attr('data-type', app.user.get('role'));
      this.$el.find('#username').text(app.user.get('name'));
    } else {
      this.$el.find('#menu').addClass('hidden');
      this.$el.find('#user').addClass('hidden');
    }
  },

  changePage: function(page) {
    console.log('change page', page);
    var title = $.isFunction(app.views[page].title) ?
                app.views[page].title() :
                app.views[page].title;
    if (app.views[page].showToolbar) {
      this.$el.find('.app-content').show();
      this.$el.find('.toolbar').show();
      this.$el.find('.toolbar-title').text(title);
      if (app.views[page].backPage === undefined) {
        this.$el.find('.toolbar-return-button').hide();
      } else {
        this.$el.find('.toolbar-return-button')
                .data('page', app.views[page].backPage)
                .show();
      }
    } else {
      this.$el.find('.app-content').hide();
      this.$el.find('.toolbar').hide();
    }
    $('.page[data-page!="' + page + '"]').hide();
    $('.page[data-page="' + page + '"]').show();
    $('title').text(title + ' – Exonum');
    this.loadingFinish();
  },

  back: function() {
    var page = this.$el.find('.toolbar-return-button').data('page');
    app.router.navigate(page, {trigger: true});
  },

  showDashboard: function() {
    app.router.navigate('dashboard', {trigger: true});
  },

  showBlockchain: function() {
    app.router.navigate("blockchain", {trigger: true});
  },

  showFlow: function() {
    app.router.navigate("flow", {trigger: true});
  },

  collapseMenu: function() {
    var navbar = $('#navbar-collapse');

    if (navbar.hasClass('in')) {
      // setTimeout is used as hack to prevent blocking of other event attached to event target
      setTimeout(function() {
        navbar.collapse('hide');
      }, 10);
    }
  },

  render: function() {
    if (this.loading) {
      this.$el.find('#loading').show();
    } else {
      this.$el.find('#loading').hide();
    }
  }

});

var FlowPage = Backbone.View.extend({
  title: "Money Flows",
  showToolbar: true,
  backPage: 'login',

  el: ".page[data-page='flow']",

  template: templates.flow,

  events: {
    'click .flow-toggle': 'toggle'
  },

  colorBase: [
    'f33d54', 'b10632', '39b54a', '22752f', 'f7941d', 'be7214', '0054a6', '172f5d',
    '095256', '087F8C', '5AAA95', '86A873', 'BB9F06', '804E49', 'E7DECD', '0A122A',
    'FFCAB1', 'ECDCB0', 'C1D7AE', '8CC084', 'A3333D', '477998', '698F3F', '7E8287'
  ],

  toggle: function(e) {
    var type = $(e.target).data('type');

    if (type) {
      app.router.navigate('flow/' + type, {trigger: true});
    } else {
      app.router.navigate('flow', {trigger: true});
    }
  },

  mouseover: function(g, bp, formatter, element) {
    bp.mouseover(element);

    g.selectAll('.mainBars').select('.flow-value')
      .text(function(d) {
        return element.part === d.part ? formatter(d) : d3.format('0.0%')(d.percent)
      });
  },

  mouseout: function(g, bp, formatter, element) {
    bp.mouseout(element);
    g.selectAll('.mainBars').select('.flow-value')
      .text(formatter);
  },

  /**
   * Get random unique color
   * @returns {string}
   */
  getColor: function(colors) {
    var color = this.colorBase[Math.floor(Math.random() * this.colorBase.length)];
    var isUnique = true;

    $.each(colors, function(i, c) {
      if (c === color) {
        isUnique = false;
        return false;
      }
    });

    return isUnique ? color : this.getColor(colors);
  },

  /**
   * Do distributor-amount-owner binding
   * @returns {Array}
   */
  collectByRevenue: function() {
    var data = [];
    $.each(app.flow.get('ownerships'), function(i, ownership) {
      var amount = ownership.amount;
      var ownerName;

      if (amount === 0) {
        return true;
      }

      $.each(app.flow.get('owners'), function(j, owner) {
        if (owner.id === ownership.id) {
          ownerName = owner.name;
          return false;
        }
      });

      $.each(app.flow.get('contracts'), function(j, contract) {
        if (contract.amount > 0 && contract.fingerprint === ownership.fingerprint) {
          $.each(app.flow.get('distributors'), function(k, distributor) {
            if (distributor.id === contract.id) {
              amount = Math.min(contract.amount, amount);
              data.push([distributor.name, ownerName, amount]);
              return false;
            }
          });
        }
      });
    });
    return data;
  },

  /**
   * Do content-amount-owner binding
   * @returns {Array}
   */
  collectByContent: function() {
    var data = [];
    $.each(app.flow.get('ownerships'), function(i, ownership) {
      var amount = ownership.amount;
      var ownerName;
      var contentTitle;

      if (amount === 0) {
        return true;
      }

      $.each(app.flow.get('owners'), function(j, owner) {
        if (owner.id === ownership.id) {
          ownerName = owner.name;
          return false;
        }
      });

      $.each(app.flow.get('contents'), function(j, content) {
        if (content.fingerprint === ownership.fingerprint) {
          contentTitle = content.title;
          return false;
        }
      });

      $.each(app.flow.get('contracts'), function(j, contract) {
        if (contract.amount > 0 && contract.fingerprint === ownership.fingerprint) {
          $.each(app.flow.get('distributors'), function(k, distributor) {
            if (distributor.id === contract.id) {
              amount = Math.min(contract.amount, amount);
              data.push([contentTitle, ownerName, amount]);
              return false;
            }
          });
        }
      });
    });
    return data;
  },

  /**
   * Do distributor-plays-content binding
   * @returns {Array}
   */
  collectByPlays: function() {
    var data = [];
    $.each(app.flow.get('contracts'), function(i, contract) {
      var plays = contract.plays;
      var distributorName;

      if (plays === 0) {
        return true;
      }

      $.each(app.flow.get('distributors'), function(j, distributor) {
        if (distributor.id === contract.id) {
          distributorName = distributor.name;
          return false;
        }
      });

      $.each(app.flow.get('contents'), function(j, content) {
        if (content.fingerprint === contract.fingerprint) {
          data.push([distributorName, content.title, plays]);
        }
      });
    });
    return data;
  },

  /**
   * Draw flow chart and insert it into view
   * @returns {boolean}
   */
  draw: function() {
    var that = this;
    var data;
    var colors = {};
    var leftTitle;
    var rightTitle;
    var valueFormatter;

    switch (app.views.flow.type) {
      case 'revenue':
        leftTitle = 'Distributors';
        rightTitle = 'Owners';
        valueFormatter = function(d) {
          return '$' + Math.round(d.value / 100).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ' ');
        };
        data = this.collectByRevenue();
        break;
      case 'content':
        leftTitle = 'Contents';
        rightTitle = 'Owners';
        valueFormatter = function(d) {
          return '$' + Math.round(d.value / 100).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ' ');
        };
        data = this.collectByContent();
        break;
      case 'plays':
        leftTitle = 'Distributors';
        rightTitle = 'Contents';
        valueFormatter = function(d) {
          return d.value.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ' ');
        };
        data = this.collectByPlays();
        break;
      default:
        return false;
    }

    // Set distributor colors
    $.each(data, function(i, element) {
      if (colors[element[0]] === undefined) {
        colors[element[0]] = that.getColor(colors);
      }
    });

    // Initialize svg object
    var chart = document.getElementById('flow-chart');
    var svg = d3.select(chart).append('svg').attr('width', 280).attr('height', 590);
    var g = svg.append('g').attr('transform', 'translate(90, 50)');
    var bp = viz.bP().data(data).min(12).pad(1).height(540).width(100).barSize(15).fill(function(d) {
      return '#' + colors[d.primary];
    });
    g.call(bp);

    // Create titles
    g.append('text').attr('x', -55).attr('y', -8).attr('class', 'flow-title').text(leftTitle);
    g.append('text').attr('x', 155).attr('y', -8).attr('class', 'flow-title').text(rightTitle);

    // Create underlines for titles
    g.append('line').attr('x1', -95).attr('x2', -15);
    g.append('line').attr('x1', 115).attr('x2', 195);

    // Add labels
    g.selectAll('.mainBars').append('text').attr('class', 'flow-label')
      .attr('x', function(d) {
        return d.part == 'primary' ? -60 : 60;
      })
      .attr('y', 6)
      .text(function(d) {
        return d.key;
      })
      .attr('text-anchor', function(d) {
        return d.part == 'primary' ? 'end' : 'start';
      });

    // Add values
    g.selectAll('.mainBars').append('text').attr('class', 'flow-value')
      .attr('x', function(d) {
        return d.part == 'primary' ? -55 : 55;
      })
      .attr('y', 6)
      .text(valueFormatter)
      .attr('text-anchor', function(d) {
        return d.part == 'primary' ? 'start' : 'end';
      });

    // Assign changes on hover
    g.selectAll('.mainBars')
      .on('mouseover', this.mouseover.bind(this, g, bp, valueFormatter))
      .on('mouseout', this.mouseout.bind(this, g, bp, valueFormatter));
  },

  getTotal: function() {
    var total = 0;
    switch (this.type) {
      case 'revenue':
        $.each(app.flow.get('contracts'), function(i, contract) {
          total += contract.amount;
        });
        break;
      case 'content':
        $.each(app.flow.get('contracts'), function(i, contract) {
          total += contract.amount;
        });
        break;
      case 'plays':
        $.each(app.flow.get('contracts'), function(i, contract) {
          total += contract.plays;
        });
        break;
      default:
        return '-'
    }
    return total;
  },

  render: function() {
    this.$el.html(this.template({
      type: this.type,
      total: this.getTotal()
    }));
    return this;
  }
});
