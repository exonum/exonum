// We teach Backbone to make crossdomain requests
Backbone.ajax = function(options) {
  options.crossDomain = true;
  options.xhrFields = {
    withCredentials: true
  };
  return Backbone.$.ajax.call(Backbone.$, options);
};

Backbone.Model.prototype.failoverSave = function(attrs, options) {
  this.save(attrs, {
    success: options.success,
    error: function(model, response) {
      if (response.status == 404 && options.retries > 0) {
        console.log('Got 404 status, retry #', options.retries);
        setTimeout(function() {
          model.failoverSave(attrs, {
            success: options.success,
            error: options.error,
            retries: options.retries - 1,
            timeout: options.timeout
          });
        }, options.timeout);
      } else {
        options.error(model, response);
      }
    }
  });
};

Backbone.Model.prototype.failoverFetch = function(options) {
  this.fetch({
    success: options.success,
    error: function(model, response) {
      if (response.status == 404 && options.retries > 0) {
        console.log('Got 404 status, retry #', options.retries);
        setTimeout(function() {
          model.failoverFetch({
            success: options.success,
            error: options.error,
            retries: options.retries - 1,
            timeout: options.timeout
          });
        }, options.timeout);
      } else {
        options.error(model, response);
      }
    }
  });
};

var Blocks = Backbone.Collection.extend({
  url: settings.api_endpoint + 'blockchain/blocks'
});

var Block = Backbone.Model.extend({
  idAttribute: 'height',
  urlRoot: settings.api_endpoint + 'blockchain/blocks'
});

var Transaction = Backbone.Model.extend({
  idAttribute: 'hash',
  urlRoot: settings.api_endpoint + 'blockchain/transactions'
});

var Auth = Backbone.Model.extend({
  url: settings.api_endpoint + 'drm/auth'
});

var Owner = Backbone.Model.extend({
  idAttribute: 'id',
  urlRoot: settings.api_endpoint + 'drm/owners'
});

var Distributor = Backbone.Model.extend({
  idAttribute: 'id',
  urlRoot: settings.api_endpoint + 'drm/distributors'
});

var Content = Backbone.Model.extend({
  idAttribute: 'fingerprint',
  urlRoot: settings.api_endpoint + 'drm/contents'
});

var Contract = Backbone.Model.extend({
  idAttribute: 'fingerprint',
  urlRoot: settings.api_endpoint + 'drm/contracts'
});

var Report = Backbone.Model.extend({
  idAttribute: 'uuid',
  urlRoot: settings.api_endpoint + 'drm/reports'
});
